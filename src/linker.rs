use anyhow::{Context, Result, anyhow};
use serde::Deserialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const RUNTIME_REPO: &str = "mysz-lang/mysz-runtime";
const RUNTIME_BINARY_DIR: &str = "binary";

fn host_compiler() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "clang"
    }

    #[cfg(not(target_os = "windows"))]
    {
        "cc"
    }
}

fn runtime_cache_root() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().context("Could not find user home directory")?;
    Ok(home_dir.join(".nibble").join("cache"))
}

#[derive(Deserialize)]
struct GithubContentEntry {
    name: String,
    download_url: Option<String>,
}

fn parse_runtime_filename(name: &str) -> Option<(u32, u32, u32)> {
    let stripped = name
        .strip_prefix("libmysz-runtime.")?
        .strip_suffix(".tar.gz")?;

    let mut parts = stripped.split('.');
    let major: u32 = parts.next()?.parse().ok()?;
    let minor: u32 = parts.next()?.parse().ok()?;
    let patch: u32 = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }

    Some((major, minor, patch))
}

fn version_string(v: (u32, u32, u32)) -> String {
    format!("{}.{}.{}", v.0, v.1, v.2)
}

fn resolve_latest_runtime() -> Result<((u32, u32, u32), String)> {
    let api_url = format!(
        "https://api.github.com/repos/{}/contents/{}",
        RUNTIME_REPO, RUNTIME_BINARY_DIR
    );

    let response = reqwest::blocking::Client::new()
        .get(&api_url)
        .header("User-Agent", "nibble-cli")
        .send()
        .context("Failed to reach GitHub API while resolving the latest mysz-runtime version")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "GitHub API returned status {} while listing mysz-runtime binaries (rate-limited?)",
            response.status()
        ));
    }

    let entries: Vec<GithubContentEntry> = response
        .json()
        .context("Failed to parse GitHub API response for mysz-runtime binaries")?;

    let best = entries
        .iter()
        .filter_map(|e| parse_runtime_filename(&e.name).map(|v| (v, e)))
        .max_by_key(|(v, _)| *v)
        .ok_or_else(|| anyhow!("No libmysz-runtime.*.tar.gz files found in the runtime repo"))?;

    let (version, entry) = best;
    let download_url = entry
        .download_url
        .clone()
        .ok_or_else(|| anyhow!("Runtime archive entry has no download_url"))?;

    Ok((version, download_url))
}

fn highest_cached_runtime(cache_root: &Path) -> Option<(u32, u32, u32)> {
    let entries = fs::read_dir(cache_root).ok()?;

    entries
        .filter_map(|e| e.ok())
        .filter_map(|e| e.file_name().to_str().map(|s| s.to_string()))
        .filter_map(|name| {
            let mut parts = name.split('.');
            let major: u32 = parts.next()?.parse().ok()?;
            let minor: u32 = parts.next()?.parse().ok()?;
            let patch: u32 = parts.next()?.parse().ok()?;
            if parts.next().is_some() {
                return None;
            }
            Some((major, minor, patch))
        })
        .max()
}

fn download_runtime_archive(download_url: &str, cache_dir: &Path) -> Result<PathBuf> {
    let lib_name = "libmysz-runtime.a";
    let target_lib_path = cache_dir.join(lib_name);

    fs::create_dir_all(cache_dir).context("Failed to create runtime cache directory")?;

    println!(
        "\x1b[1;36mDownloading\x1b[0m mysz-runtime from {}...",
        download_url
    );

    let response = reqwest::blocking::get(download_url)
        .context("Connection failed while downloading the mysz-runtime archive")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to download mysz-runtime archive. Server responded with status code: {}",
            response.status()
        ));
    }

    let tar_gz = flate2::read::GzDecoder::new(response);
    let mut archive = tar::Archive::new(tar_gz);

    archive
        .unpack(cache_dir)
        .context("Corrupt compression framework encountered while unpacking runtime tarball")?;

    if !target_lib_path.exists() {
        return Err(anyhow!(
            "Download completed successfully, but expected static asset '{}' was missing inside the archive.",
            lib_name
        ));
    }

    Ok(target_lib_path)
}

fn fetch_runtime() -> Result<PathBuf> {
    let cache_root = runtime_cache_root()?;

    match resolve_latest_runtime() {
        Ok((version, download_url)) => {
            let cache_dir = cache_root.join(version_string(version));
            let target_lib_path = cache_dir.join("libmysz-runtime.a");

            if target_lib_path.exists() {
                return Ok(target_lib_path);
            }

            download_runtime_archive(&download_url, &cache_dir)
        }
        Err(resolve_err) => {
            if let Some(cached_version) = highest_cached_runtime(&cache_root) {
                eprintln!(
                    "\x1b[1;33mWarning:\x1b[0m could not resolve the latest mysz-runtime ({}), using cached v{}",
                    resolve_err,
                    version_string(cached_version)
                );
                let cache_dir = cache_root.join(version_string(cached_version));
                Ok(cache_dir.join("libmysz-runtime.a"))
            } else {
                Err(resolve_err
                    .context("and no cached mysz-runtime is available locally to fall back on"))
            }
        }
    }
}

pub fn link_binary(
    obj_paths: &[PathBuf],
    output_exe: &Path,
    noruntime: bool,
    link_files: &[PathBuf],
) -> Result<()> {
    create_output_parent(output_exe)?;

    let mut args = Vec::new();

    for obj in obj_paths {
        args.push(obj.to_string_lossy().into_owned());
    }

    if !noruntime {
        let runtime_lib_path = fetch_runtime().context("Runtime layer alignment failed")?;

        args.push(runtime_lib_path.to_string_lossy().into_owned());
    }

    for file in link_files {
        if !file.exists() {
            return Err(anyhow!("Link parameter target path not found: {:?}", file));
        }

        args.push(file.to_string_lossy().into_owned());
    }

    args.push("-o".into());
    args.push(output_exe.to_string_lossy().into_owned());

    run_linker(&args)
}

pub fn link_shared(obj_paths: &[PathBuf], output: &Path, link_files: &[PathBuf]) -> Result<()> {
    create_output_parent(output)?;

    let mut args = Vec::new();

    args.push("-shared".into());

    for obj in obj_paths {
        args.push(obj.to_string_lossy().into_owned());
    }

    for file in link_files {
        if !file.exists() {
            return Err(anyhow!("Link parameter target path not found: {:?}", file));
        }

        args.push(file.to_string_lossy().into_owned());
    }

    args.push("-o".into());
    args.push(output.to_string_lossy().into_owned());

    run_linker(&args)
}

fn create_output_parent(output: &Path) -> Result<()> {
    if let Some(parent) = output.parent()
        && !parent.as_os_str().is_empty()
    {
        fs::create_dir_all(parent).with_context(|| {
            format!(
                "Failed to create output destination directory: {:?}",
                parent
            )
        })?;
    }

    Ok(())
}

fn run_linker(args: &[String]) -> Result<()> {
    let compiler = host_compiler();

    let output = Command::new(compiler)
        .args(args)
        .output()
        .with_context(|| {
            format!(
                "Platform linker failed to execute. Verify '{}' is installed and available in PATH.",
                compiler
            )
        })?;

    if output.status.success() {
        Ok(())
    } else {
        let stderr_msg = String::from_utf8_lossy(&output.stderr);

        Err(anyhow!("Host platform linker failed:\n{}", stderr_msg))
    }
}
