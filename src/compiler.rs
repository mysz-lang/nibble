use crate::linker;
use crate::packages;
use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tempfile::Builder;

pub struct Pipeline {
    input: Vec<PathBuf>,
    output: PathBuf,
    _optimize: bool,
    noruntime: bool,
    link_files: Vec<PathBuf>,
    include_paths: Vec<PathBuf>,
}

impl Pipeline {
    pub fn new(
        input: Vec<PathBuf>,
        output: PathBuf,
        _optimize: bool,
        noruntime: bool,
        link_files: Vec<PathBuf>,
        include: Vec<PathBuf>,
    ) -> Self {
        let mut include_paths = include;

        if let Err(e) = packages::resolve_local_manifest() {
            eprintln!("\x1b[1;33mManifest Resolution Warning:\x1b[0m {:?}", e);
        }

        if let Ok(packs_dir) = packages::get_packs_dir() {
            include_paths.push(packs_dir);
        }

        if include_paths.is_empty() {
            if let Ok(val) = std::env::var("NIBBLE_PATH") {
                include_paths.push(PathBuf::from(val));
            }
            include_paths.push(PathBuf::from("."));
        }

        Self {
            input,
            output,
            _optimize,
            noruntime,
            link_files,
            include_paths,
        }
    }

    pub fn compile(&self, jsonout: bool) -> Result<()> {
        let tmp_dir = Builder::new().prefix("nibble-build-").tempdir()?;
        let mut object_files = Vec::new();

        println!("\x1b[1;34mCompiling\x1b[0m targets with mysz-core engine...");

        for (i, input) in self.input.iter().enumerate() {
            let obj_path = tmp_dir.path().join(format!("{}.o", i));

            mysz_core::compile_file(
                input,
                obj_path
                    .to_str()
                    .context("Temporary object storage tracking allocation path could not yield valid UTF-8 symbols conversion formatting attributes")?,
                &self.include_paths,
                jsonout
            )
            .map_err(|e| anyhow!("Mysz compiler core error:\n{}", e))?;

            object_files.push(obj_path);
        }

        println!("\x1b[1;34mLinking\x1b[0m platform objects...");

        linker::link_binary(
            &object_files,
            &self.output,
            self.noruntime,
            &self.link_files,
        )?;

        Ok(())
    }

    pub fn run_ephemeral(input: PathBuf, include: Vec<PathBuf>, jsonout: bool) -> Result<()> {
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
        );
        pipeline.compile(jsonout)?;

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

        // Always use JSON for machine‑readable output.
        mysz_core::check_file(&input, &include_paths, true)
            .map_err(|e| anyhow!("{}", e))
    }
}
