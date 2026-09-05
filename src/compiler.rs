use crate::linker;
use crate::out::ResultType;
use crate::packages;
use anyhow::{Context, Result, anyhow};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::Builder;

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
            include_paths.push(packs_dir);
        }

        if include_paths.is_empty() {
            if let Ok(val) = std::env::var("NIBBLE_PATH") {
                include_paths.push(PathBuf::from(val));
            }

            include_paths.push(PathBuf::from("."));
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
        })
    }

    pub fn compile(&self) -> Result<()> {
        let tmp_dir = Builder::new().prefix("nibble-build-").tempdir()?;

        let mut object_files = Vec::new();

        println!("\x1b[1;34mCompiling\x1b[0m targets with mysz-core engine...");

        for (i, input) in self.input.iter().enumerate() {
            let obj_path = tmp_dir.path().join(format!("{}.o", i));

            let target = self.compiler.target()?;

            let ctx = CompilerCtx::new(
                input,
                &self.include_paths,
                self.compiler.output_json,
                target,
            );

            mysz_core::compile_file(
                ctx,
                obj_path
                    .to_str()
                    .context("Temporary object path is not valid UTF-8")?,
            )
            .map_err(|e| anyhow!("Mysz compiler core error:\n{}", e))?;

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

        let ctx = CompilerCtx::new(input, &include_paths, true, target);

        mysz_core::check_file(ctx).map_err(|e| anyhow!("{}", e))
    }
}
