use crate::linker;
use crate::out::ResultType;
use crate::packages;
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::{PathBuf};
use std::process::Command;
use tempfile::Builder;

use mysz_core::athelp::ATBuilder;
use mysz_core::compiler::{check_at, compile_at_graph};
use mysz_core::utils::ats::{ATEntry, ATInfo};
use mysz_core::utils::ctx::CompilerCtx;

pub struct Pipeline {
    input: Vec<PathBuf>,
    output: PathBuf,
    _optimize: bool,
    noruntime: bool,
    link_files: Vec<PathBuf>,
    include_paths: Vec<PathBuf>,
    compiler: packages::CompilerConfig,
    result: ResultType,
    dependency_names: Vec<String>,      // names of dependencies for the main AT
    dependency_ats: Vec<ATInfo>,        // ATs for packages
}

impl Pipeline {
    pub fn new(
        input: Vec<PathBuf>,
        output: PathBuf,
        _optimize: bool,
        noruntime: bool,
        link_files: Vec<PathBuf>,
        include: Vec<PathBuf>,
        result: ResultType,
    ) -> Result<Self> {
        let mut include_paths = include;

        packages::resolve_local_manifest()?;

        let compiler = packages::compiler_config()?;

        if let Ok(packs_dir) = packages::get_packs_dir() {
            include_paths.push(packs_dir.clone());
        }

        if include_paths.is_empty() {
            if let Ok(val) = std::env::var("NIBBLE_PATH") {
                include_paths.push(PathBuf::from(val));
            }

            include_paths.push(PathBuf::from("."));
        }

        // Read dependencies from manifest.
        let mut dependency_names = Vec::new();
        let mut dependency_ats = Vec::new();
        if let Some(manifest) = packages::load_manifest()? {
            if let Some(deps) = manifest.dependencies {
                for (name, _source) in deps {
                    dependency_names.push(name.clone());
                    if let Ok(at) = Self::build_package_at(&name) {
                        dependency_ats.push(at);
                    } else {
                        eprintln!("Warning: could not build @'{}'", name);
                    }
                }
            }
        }

        Ok(Self {
            input,
            output,
            _optimize,
            noruntime,
            link_files,
            include_paths,
            compiler,
            result,
            dependency_names,
            dependency_ats,
        })
    }

    /// Build an ATInfo for a single file using ATBuilder.
    fn build_at_from_file(
        file_path: &PathBuf,
        dependencies: Vec<String>,
    ) -> Result<ATInfo, anyhow::Error> {
        let name = file_path
            .file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow!("Invalid file name: {}", file_path.display()))?
            .to_string();
        let root = file_path
            .parent()
            .ok_or_else(|| anyhow!("File has no parent directory: {}", file_path.display()))?
            .to_path_buf();

        let mut builder = ATBuilder::new()
            .name(name.clone())
            .root_dir(root)
            .entry_file(file_path.clone())
            .add_file(file_path.clone(), vec![name]);

        for dep in dependencies {
            builder = builder.add_dependency(dep, None);
        }

        builder
            .build()
            .map_err(|e| anyhow!("Failed to build @{}", e))
    }

    /// Build an ATInfo for a package (dependency) from the packs directory.
    fn build_package_at(package_name: &str) -> Result<ATInfo, anyhow::Error> {
        let packs_dir = packages::get_packs_dir()
            .map_err(|e| anyhow!("Failed to get packs directory: {}", e))?;
        let pkg_dir = packs_dir.join(package_name);

        if !pkg_dir.exists() || !pkg_dir.is_dir() {
            return Err(anyhow!(
                "Package directory for '{}' does not exist at {:?}",
                package_name,
                pkg_dir
            ));
        }

        let at = ATBuilder::new()
            .name(package_name)
            .root_dir(pkg_dir)
            .discover_files()
            .map_err(|e| {
                anyhow!(
                    "Failed to discover files for package '{}': {}",
                    package_name,
                    e
                )
            })?
            .build()
            .map_err(|e| anyhow!("Failed to build @{}: {}", package_name, e))?;

        Ok(at)
    }

    pub fn compile(&self) -> Result<()> {
        let tmp_dir = Builder::new().prefix("nibble-build-").tempdir()?;

        let mut object_files = Vec::new();

        println!("\x1b[1;34mCompiling\x1b[0m targets...");

        for (i, input) in self.input.iter().enumerate() {
            let obj_path = tmp_dir.path().join(format!("{}.o", i));

            // Build AT for the main input file, passing the dependency names.
            let main_at = Self::build_at_from_file(input, self.dependency_names.clone())?;

            let mut all_ats = vec![main_at];
            all_ats.extend(self.dependency_ats.clone());

            let entry = ATEntry {
                info: 0,
                is_current: true,
            };

            let target = self.compiler.target()?;
            let ctx = CompilerCtx::new(
                input,
                &self.include_paths,
                self.compiler.output_json,
                target,
            );

            compile_at_graph(
                &ctx,
                &all_ats,
                &entry,
                obj_path
                    .to_str()
                    .context("Temporary object path is not valid UTF-8")?,
            )
            .map_err(|e| anyhow!("Mysz core error:\n{}", e))?;

            object_files.push(obj_path);
        }

        match self.result {
            ResultType::Binary => {
                println!("\x1b[1;34mLinking\x1b[0m platform objects...");

                linker::link_binary(
                    &object_files,
                    &self.output,
                    self.noruntime,
                    &self.link_files,
                )?;
            }

            ResultType::Object => {
                println!("\x1b[1;34mOutputting\x1b[0m object...");

                if object_files.len() != 1 {
                    return Err(anyhow!(
                        "Object output currently requires exactly one input file"
                    ));
                }

                fs::copy(&object_files[0], &self.output).with_context(|| {
                    format!("Failed to write object file to {}", self.output.display())
                })?;
            }

            ResultType::Shared => {
                println!("\x1b[1;34mLinking\x1b[0m shared library...");

                linker::link_shared(&object_files, &self.output, &self.link_files)?;
            }
        }

        Ok(())
    }

    pub fn run_ephemeral(input: PathBuf, include: Vec<PathBuf>) -> Result<()> {
        let target_exe = if cfg!(target_os = "windows") {
            "ephemeral_run.exe"
        } else {
            "./ephemeral_run"
        };

        let target_path = PathBuf::from(target_exe);

        let pipeline = Self::new(
            vec![input],
            target_path.clone(),
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

    pub fn check(input: PathBuf, include: Vec<PathBuf>) -> Result<()> {
        let mut include_paths = include;

        if let Ok(packs_dir) = packages::get_packs_dir() {
            include_paths.push(packs_dir);
        }

        if include_paths.is_empty() {
            if let Ok(val) = std::env::var("NIBBLE_PATH") {
                include_paths.push(PathBuf::from(val));
            }

            include_paths.push(PathBuf::from("."));
        }

        let compiler = packages::compiler_config()?;
        let target = compiler.target()?;

        // Read dependencies again for the check command.
        let mut dependency_names = Vec::new();
        let mut all_ats = Vec::new();
        if let Some(manifest) = packages::load_manifest()? {
            if let Some(deps) = manifest.dependencies {
                for (name, _source) in deps {
                    dependency_names.push(name.clone());
                    if let Ok(at) = Self::build_package_at(&name) {
                        all_ats.push(at);
                    }
                }
            }
        }

        let main_at = Self::build_at_from_file(&input, dependency_names)?;
        all_ats.insert(0, main_at); // main AT first

        let entry = ATEntry {
            info: 0,
            is_current: true,
        };

        let ctx = CompilerCtx::new(&input, &include_paths, true, target);

        check_at(&ctx, &all_ats, &entry).map_err(|e| anyhow!("{}", e))
    }
}