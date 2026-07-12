use anyhow::{anyhow, Context, Result};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RUNTIME_VERSION: &str = "0.3.0";
const REPO_URL: &str = "https://raw.githubusercontent.com/mysz-lang/mysz-runtime/main/binary";

fn fetch_runtime() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().context("Could not find user home directory")?;
    let cache_dir = home_dir.join(".nibble").join("cache").join(RUNTIME_VERSION);

    let lib_name = "libmysz-runtime.a";
    let target_lib_path = cache_dir.join(lib_name);

    if target_lib_path.exists() {
        return Ok(target_lib_path);
    }

    fs::create_dir_all(&cache_dir).context("Failed to create runtime cache directory")?;

    let archive_name = format!("libmysz-runtime.{}.tar.gz", RUNTIME_VERSION);
    let download_url = format!("{}/{}", REPO_URL, archive_name);

    println!(
        "\x1b[1;36mDownloading\x1b[0m mysz-runtime v{} from remote repo...",
        RUNTIME_VERSION
    );

    let response = reqwest::blocking::get(&download_url)
        .context("Connection failed. Check your network link to the GitHub runtime repository")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to pull runtime library binary. Server responded with status code: {}",
            response.status()
        ));
    }

    let tar_gz = flate2::read::GzDecoder::new(response);
    let mut archive = tar::Archive::new(tar_gz);
    archive
        .unpack(&cache_dir)
        .context("Corrupt compression framework encountered while unpacking runtime tarball")?;

    if !target_lib_path.exists() {
        return Err(anyhow!(
            "Download completed successfully, but expected static asset '{}' was missing inside the archive context.",
            lib_name
        ));
    }

    Ok(target_lib_path)
}

pub fn link_binary(
    obj_paths: &[PathBuf],
    output_exe: &Path,
    noruntime: bool,
    link_files: &[PathBuf],
) -> Result<()> {
    // CRITICAL FIX: Ensure the target output directory exists before generating output files
    if let Some(parent) = output_exe.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| {
                format!(
                    "Failed to create output binary destination layout at path: {:?}",
                    parent
                )
            })?;
        }
    }

    let mut args = Vec::new();

    for obj in obj_paths {
        args.push(obj.to_string_lossy().into_owned());
    }

    let temp_runtime = "nibble_runtime.c";

    if !noruntime {
        let runtime_lib_path = fetch_runtime().context("Runtime layer alignment failed")?;
        args.push(runtime_lib_path.to_string_lossy().into_owned());
    }

    for file in link_files {
        if !file.exists() {
            if !noruntime {
                let _ = fs::remove_file(temp_runtime);
            }
            return Err(anyhow!("Link parameter target path not found: {:?}", file));
        }

        args.push(file.to_string_lossy().into_owned());
    }

    args.push("-o".into());
    args.push(output_exe.to_string_lossy().into_owned());

    #[cfg(target_os = "windows")]
    let compiler = "clang";
    #[cfg(not(target_os = "windows"))]
    let compiler = "cc";

    let output = Command::new(compiler)
        .args(&args)
        .output()
        .with_context(|| format!("Platform system linker failed execution. Verify '{}' is installed and added to your path environment variables.", compiler))?;

    if !noruntime {
        let _ = fs::remove_file(temp_runtime);
    }

    if output.status.success() {
        Ok(())
    } else {
        let stderr_msg = String::from_utf8_lossy(&output.stderr);
        Err(anyhow!(
            "Compilation phase interrupted by host platform compiler linkage pipeline:\n{}",
            stderr_msg
        ))
    }
}
