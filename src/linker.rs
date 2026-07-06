use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use anyhow::{Context, Result, anyhow};

pub fn link_binary(obj_path: &Path, output_exe: &Path, noruntime: bool, link_files: &[PathBuf]) -> Result<()> {
    let mut args = vec![obj_path.to_str().unwrap().to_string()];
    let temp_runtime = "nibble_abi_runtime.c";

    if !noruntime {
        let runtime_source = include_str!("runtime.c");
        fs::write(temp_runtime, runtime_source).context("Failed to dump embedded ABI source")?;
        args.push(temp_runtime.to_string());
    }

    for file in link_files {
        if !file.exists() {
            if !noruntime { let _ = fs::remove_file(temp_runtime); }
            return Err(anyhow!("Link file dependency target not found: {:?}", file));
        }
        args.push(file.to_string_lossy().into_owned());
    }

    args.push("-o".to_string());
    args.push(output_exe.to_str().unwrap().to_string());

    #[cfg(target_os = "windows")]
    let compiler = "clang";
    #[cfg(not(target_os = "windows"))]
    let compiler = "cc";

    let output = Command::new(compiler)
        .args(&args)
        .output()
        .with_context(|| format!("System linker error. Verify '{}' is installed.", compiler))?;

    if !noruntime {
        let _ = fs::remove_file(temp_runtime);
    }

    if output.status.success() {
        Ok(())
    } else {
        let stderr_msg = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!("Linker phase terminated abruptly:\n{}", stderr_msg))
    }
}