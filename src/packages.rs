use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Deserialize, Debug, Clone)]
#[serde(untagged)]
pub enum DependencySource {
    Named(String),
    Custom {
        source: String,
        root_dir: String,
        archive_prefix: Option<String>,
    },
}

#[derive(Deserialize, Debug)]
pub struct Manifest {
    pub dependencies: Option<HashMap<String, DependencySource>>,
}

#[derive(Clone)]
struct PackageRegistryInfo {
    tarball_url: String,
    archive_prefix: String,
    root_dir: String,
}

fn get_default_registry() -> HashMap<&'static str, PackageRegistryInfo> {
    let mut registry = HashMap::new();
    registry.insert(
        "std",
        PackageRegistryInfo {
            tarball_url: "https://github.com/mysz-lang/mysz-std/archive/refs/heads/main.tar.gz"
                .to_string(),
            archive_prefix: "mysz-std-main".to_string(),
            root_dir: "src".to_string(),
        },
    );
    registry
}

pub fn get_packs_dir() -> Result<PathBuf> {
    let home_dir = dirs::home_dir().context("Could not find user home directory")?;
    Ok(home_dir.join(".nibble").join("packs"))
}

pub fn resolve_local_manifest() -> Result<()> {
    let manifest_path = Path::new("nibble.toml");
    if !manifest_path.exists() {
        return Ok(());
    }

    let content = fs::read_to_string(manifest_path).with_context(|| {
        format!(
            "Failed to read manifest file structure from {:?}",
            manifest_path
        )
    })?;

    let manifest: Manifest = toml::from_str(&content).context(
        "Syntax or configuration error inside your local 'nibble.toml' manifest definition",
    )?;

    if let Some(deps) = manifest.dependencies {
        for (name, source) in deps {
            install_package(&name, &source)?;
        }
    }
    Ok(())
}

pub fn install_package(package_alias: &str, source: &DependencySource) -> Result<()> {
    let packs_dir = get_packs_dir()?;
    let target_pkg_base = packs_dir.join(package_alias);

    if target_pkg_base.exists() && fs::read_dir(&target_pkg_base)?.next().is_some() {
        return Ok(());
    }

    let target_info = match source {
        DependencySource::Named(registry_name) => {
            let registry = get_default_registry();
            registry
                .get(registry_name.as_str())
                .cloned()
                .ok_or_else(|| anyhow!("Package identity shortcut '{}' is missing from the global default package registry registry.", registry_name))?
        }
        DependencySource::Custom {
            source,
            root_dir,
            archive_prefix,
        } => {
            let prefix = archive_prefix.clone().unwrap_or_else(|| {
                source
                    .split('/')
                    .last()
                    .unwrap_or("archive")
                    .replace(".tar.gz", "")
                    .replace(".zip", "")
            });
            PackageRegistryInfo {
                tarball_url: source.clone(),
                archive_prefix: prefix,
                root_dir: root_dir.clone(),
            }
        }
    };

    println!(
        "\x1b[1;36mDownloading\x1b[0m dependency '{}'...",
        package_alias
    );

    let response = reqwest::blocking::get(&target_info.tarball_url)
        .with_context(|| format!("Network Connection Failure: Unable to pull remote tarball archive package target for dependency package '{}'. Double check your internet access setup.", package_alias))?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Repository network server endpoint tracking '{}' returned failure status code: {}",
            package_alias,
            response.status()
        ));
    }

    let tar_gz = flate2::read::GzDecoder::new(response);
    let mut archive = tar::Archive::new(tar_gz);
    let mut extracted_count = 0;

    for entry_result in archive
        .entries()
        .context("Failed to decode tar data chunk frames stream payload context")?
    {
        let mut entry = entry_result
            .context("Corrupt binary payload segment detected inside download bundle")?;
        let path = entry
            .path()
            .context("Missing package entry path reference attributes")?
            .to_path_buf();
        let components: Vec<_> = path.components().collect();

        if components.len() < 2 {
            continue;
        }

        let first_dir = components[0].as_os_str().to_string_lossy();
        if !first_dir.contains(&target_info.archive_prefix)
            && first_dir != target_info.archive_prefix
        {
            continue;
        }

        let second_dir = components[1].as_os_str().to_string_lossy();
        if second_dir != target_info.root_dir {
            continue;
        }

        let relative_components: Vec<_> = components.iter().skip(2).collect();
        if relative_components.is_empty() {
            continue;
        }

        let mut final_relative_path = PathBuf::new();
        for comp in relative_components {
            final_relative_path.push(comp);
        }

        let out_file_path = target_pkg_base.join(&final_relative_path);

        if let Some(parent) = out_file_path.parent() {
            fs::create_dir_all(parent).context(
                "Failed to initialize target output system folders hierarchy mapping requirements",
            )?;
        }

        if entry.header().entry_type().is_file() {
            entry.unpack(&out_file_path).with_context(|| {
                format!("Failed parsing compression allocation targets to folder storage destination: {:?}", out_file_path)
            })?;
            extracted_count += 1;
        }
    }

    if extracted_count == 0 {
        return Err(anyhow!(
            "Archive downloaded successfully, but zero files matched your designated 'root_dir = \"{}\"' path filter within prefix layout framework context '{}'.",
            target_info.root_dir, target_info.archive_prefix
        ));
    }

    println!(
        "\x1b[1;32mInstalled\x1b[0m dependency '{}' successfully ({} files extracted).",
        package_alias, extracted_count
    );

    Ok(())
}
