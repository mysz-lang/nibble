use std::fs;
use std::path::{PathBuf};
use std::process::Command;
use anyhow::{Context, Result, anyhow};
use tempfile::Builder;
use crate::linker;

pub struct Pipeline {
    input: PathBuf,
    output: PathBuf,
    _optimize: bool,
    noruntime: bool,
    link_files: Vec<PathBuf>,
}

impl Pipeline {
    pub fn new(input: PathBuf, output: PathBuf, _optimize: bool, noruntime: bool, link_files: Vec<PathBuf>) -> Self {
        Self { input, output, _optimize, noruntime, link_files }
    }

    pub fn compile(&self) -> Result<()> {
        let source_code = fs::read_to_string(&self.input)?;

        let tmp_dir = Builder::new().prefix("nibble-build-").tempdir()?;
        let obj_file_path = tmp_dir.path().join("output.o");

        println!("\x1b[1;34mCompiling\x1b[0m targets with mysz-core engine...");
        mysz_core::compile_source(&source_code, obj_file_path.to_str().unwrap())
            .map_err(|e| anyhow!("Mysz compiler core error:\n{}", e))?;

        println!("\x1b[1;34mLinking\x1b[0m platform objects...");
        
        linker::link_binary(&obj_file_path, &self.output, self.noruntime, &self.link_files)?;

        Ok(())
    }

    pub fn run_ephemeral(input: PathBuf) -> Result<()> {
        let target_exe = if cfg!(target_os = "windows") { "ephemeral_run.exe" } else { "./ephemeral_run" };
        let target_path = PathBuf::from(target_exe);

        let pipeline = Self::new(input, target_path.clone(), false, false, Vec::new());
        pipeline.compile()?;

        println!("\x1b[1;34mExecuting\x1b[0m application binary loop...");
        let mut child = Command::new(&target_exe)
            .spawn()
            .with_context(|| format!("Failed to spawn native run instance: {}", target_exe))?;

        let exit_status = child.wait()?;
        
        let _ = fs::remove_file(target_path);

        if exit_status.success() {
            Ok(())
        } else {
            Err(anyhow!("Target application exited with non-zero code: {:?}", exit_status.code()))
        }
    }
}