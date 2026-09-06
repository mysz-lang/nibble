use crate::linker;
use crate::out::ResultType;
use crate::packages;
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mysz_core::athelp::ATBuilder;
use mysz_core::compiler::{check_at, compile_at_graph};
use mysz_core::utils::ats::{ATEntry, ATInfo};
use mysz_core::utils::ctx::CompilerCtx;

pub struct Pipeline {
    output: PathBuf,
    _optimize: bool,
    noruntime: bool,
    link_files: Vec<PathBuf>,
    include_paths: Vec<PathBuf>,
    compiler: packages::CompilerConfig,
    result: ResultType,
    at_metadata: packages::AtMetadata,
    dependencies: Vec<packages::Dependency>,
    dependency_ats: Vec<ATInfo>,
}

impl Pipeline {
    pub fn new(
        output_override: Option<PathBuf>,
        _optimize: bool,
        noruntime: bool,
        link_files: Vec<PathBuf>,
        include: Vec<PathBuf>,
        result: ResultType,
    ) -> Result<Self> {
        let mut include_paths = include;

        packages::resolve_local_manifest()?;

        let manifest = packages::require_manifest()?;
        let at_metadata = manifest.at;
        let compiler = manifest.compiler;
        let dependencies = manifest.dependencies;

        include_paths.push(packages::packs_dir());
        include_paths.push(PathBuf::from("."));

        let mut dependency_ats = Vec::new();
        for dep in &dependencies {
            match Self::build_package_at(dep) {
                Ok((at, comppath)) => {
                    dependency_ats.push(at);
                    include_paths.push(comppath);
                }
                Err(e) => eprintln!("Warning: could not build @'{}': {}", dep.alias, e),
            }
        }

        let output = match output_override {
            Some(path) => path,
            None => PathBuf::from(at_metadata.output_name()),
        };

        Ok(Self {
            output,
            _optimize,
            noruntime,
            link_files,
            include_paths,
            compiler,
            result,
            at_metadata,
            dependencies,
            dependency_ats,
        })
    }

    fn build_project_at(
        at_metadata: &packages::AtMetadata,
        dependencies: &[packages::Dependency],
    ) -> Result<ATInfo, anyhow::Error> {
        let root = PathBuf::from(".");

        let mut builder = ATBuilder::new()
            .name(at_metadata.name.clone())
            .root_dir(root.clone());

        let entry_file = if let Some(entry) = &at_metadata.entry {
            let path = root.join(entry);

            if !path.exists() {
                return Err(anyhow!(
                    "Entry file '{}' (from [at].entry in manifest.nibble) was not found",
                    entry
                ));
            }

            path
        } else {
            root.join("main.mysz")
        };

        builder = builder.entry_file(entry_file.clone());

        fn visit_project_dir(
            dir: &Path,
            root: &Path,
            builder: &mut ATBuilder,
        ) -> Result<(), anyhow::Error> {
            for entry in fs::read_dir(dir)
                .with_context(|| format!("Failed to read directory {}", dir.display()))?
            {
                let entry = entry?;
                let path = entry.path();

                if path.is_dir() {
                    if path.file_name().and_then(|n| n.to_str()) == Some("nout") {
                        continue;
                    }

                    visit_project_dir(&path, root, builder)?;
                } else if path.extension().and_then(|e| e.to_str()) == Some("mysz") {
                    let relative = path.strip_prefix(root).with_context(|| {
                        format!(
                            "Failed to make '{}' relative to '{}'",
                            path.display(),
                            root.display()
                        )
                    })?;

                    let mut module_path = Vec::new();

                    if let Some(parent) = relative.parent() {
                        for component in parent.components() {
                            module_path.push(component.as_os_str().to_string_lossy().into_owned());
                        }
                    }

                    if let Some(stem) = path.file_stem() {
                        module_path.push(stem.to_string_lossy().into_owned());
                    }

                    *builder = std::mem::take(builder).add_file(path.clone(), module_path);
                }
            }

            Ok(())
        }

        visit_project_dir(&root, &root, &mut builder)?;

        for dep in dependencies {
            builder = builder.add_dependency(dep.alias.clone(), dep.version.clone());
        }

        builder
            .build()
            .map_err(|e| anyhow!("Failed to build @{}: {}", at_metadata.name, e))
    }

    fn build_package_at(dep: &packages::Dependency) -> Result<(ATInfo, PathBuf), anyhow::Error> {
        let pkg_dir = packages::packs_dir().join(&dep.alias);

        if !pkg_dir.exists() || !pkg_dir.is_dir() {
            return Err(anyhow!(
                "Package directory for '{}' does not exist at {:?} (dependency resolution ran but produced nothing?)",
                dep.alias,
                pkg_dir
            ));
        }

        let dep_manifest_path = pkg_dir.join("manifest.nibble");
        let dep_entry = if dep_manifest_path.exists() {
            let content = fs::read_to_string(&dep_manifest_path)
                .with_context(|| format!("Failed to read manifest.nibble for '{}'", dep.alias))?;
            packages::parse_manifest(&content)
                .with_context(|| format!("Invalid manifest.nibble for '{}'", dep.alias))?
                .at
                .entry
        } else {
            None
        };

        let source_root = match &dep_entry {
            Some(entry) => {
                let entry_path = Path::new(entry);
                match entry_path.parent() {
                    Some(parent) if !parent.as_os_str().is_empty() => pkg_dir.join(parent),
                    _ => pkg_dir.clone(),
                }
            }
            None => pkg_dir.clone(),
        };

        let mut builder = ATBuilder::new()
            .name(dep.alias.clone())
            .root_dir(source_root.clone());

        if let Some(entry) = &dep_entry {
            builder = builder.entry_file(pkg_dir.join(entry));
        }

        let at = builder
            .discover_files()
            .map_err(|e| {
                anyhow!(
                    "Failed to discover files for package '{}': {}",
                    dep.alias,
                    e
                )
            })?
            .build()
            .map_err(|e| anyhow!("Failed to build @{}: {}", dep.alias, e))?;

        Ok((at, source_root))
    }

    pub fn compile(&self) -> Result<()> {
        let obj_dir = packages::build_dir();
        fs::create_dir_all(&obj_dir).with_context(|| format!("Failed to create {:?}", obj_dir))?;
        let obj_path = obj_dir.join("out.o");

        println!(
            "\x1b[1;34mCompiling\x1b[0m project '{}'...",
            self.at_metadata.name
        );

        let project_at = Self::build_project_at(&self.at_metadata, &self.dependencies)?;

        let mut all_ats = vec![project_at];
        all_ats.extend(self.dependency_ats.clone());

        let entry = ATEntry {
            info: 0,
            is_current: true,
        };

        let entry_file_path = self
            .at_metadata
            .entry
            .as_ref()
            .map(|e| PathBuf::from(".").join(e))
            .unwrap_or_else(|| PathBuf::from("."));

        let target = self.compiler.target()?;
        let ctx = CompilerCtx::new(&entry_file_path, &self.include_paths, false, target);

        compile_at_graph(
            &ctx,
            &all_ats,
            &entry,
            obj_path
                .to_str()
                .context("Object path is not valid UTF-8")?,
        )
        .map_err(|e| anyhow!("Mysz core error:\n{}", e))?;

        match self.result {
            ResultType::Binary => {
                println!("\x1b[1;34mLinking\x1b[0m platform objects...");

                linker::link_binary(&[obj_path], &self.output, self.noruntime, &self.link_files)?;
            }

            ResultType::Object => {
                println!("\x1b[1;34mOutputting\x1b[0m object...");

                fs::copy(&obj_path, &self.output).with_context(|| {
                    format!("Failed to write object file to {}", self.output.display())
                })?;
            }

            ResultType::Shared => {
                println!("\x1b[1;34mLinking\x1b[0m shared library...");

                linker::link_shared(&[obj_path], &self.output, &self.link_files)?;
            }
        }

        Ok(())
    }

    pub fn run_ephemeral(include: Vec<PathBuf>) -> Result<()> {
        let target_exe = if cfg!(target_os = "windows") {
            "ephemeral_run.exe"
        } else {
            "./ephemeral_run"
        };

        let target_path = PathBuf::from(target_exe);

        let pipeline = Self::new(
            Some(target_path.clone()),
            false,
            false,
            Vec::new(),
            include,
            ResultType::Binary,
        )?;

        pipeline.compile()?;

        println!("\x1b[1;34mExecuting\x1b[0m application binary loop...");

        let mut child = Command::new(target_exe).spawn().with_context(|| {
            format!(
                "Failed to spawn native run instance execution handle at: {}",
                target_exe
            )
        })?;

        let exit_status = child.wait()?;

        let _ = fs::remove_file(target_path);

        if exit_status.success() {
            Ok(())
        } else {
            Err(anyhow!(
                "Application run closed unexpectedly with bad condition result exit status execution frame identifier: {:?}",
                exit_status.code()
            ))
        }
    }

    pub fn check(include: Vec<PathBuf>) -> Result<()> {
        let mut include_paths = include;

        packages::resolve_local_manifest()?;

        let manifest = packages::require_manifest()?;
        let at_metadata = manifest.at;
        let compiler = manifest.compiler;
        let target = compiler.target()?;

        include_paths.push(packages::packs_dir());
        include_paths.push(PathBuf::from("."));

        let mut all_ats = Vec::new();
        for dep in &manifest.dependencies {
            match Self::build_package_at(dep) {
                Ok((at, comppath)) => {
                    all_ats.push(at);
                    include_paths.push(comppath);
                }
                Err(e) => eprintln!("Warning: could not build @'{}': {}", dep.alias, e),
            }
        }

        let project_at = Self::build_project_at(&at_metadata, &manifest.dependencies)?;
        all_ats.insert(0, project_at); // project AT first

        let entry = ATEntry {
            info: 0,
            is_current: true,
        };

        let entry_file_path = at_metadata
            .entry
            .as_ref()
            .map(|e| PathBuf::from(".").join(e))
            .unwrap_or_else(|| PathBuf::from("."));

        let ctx = CompilerCtx::new(&entry_file_path, &include_paths, true, target);

        check_at(&ctx, &all_ats, &entry).map_err(|e| anyhow!("{}", e))
    }
}
