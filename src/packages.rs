use anyhow::{Context, Result, anyhow};
use kdl::{KdlDocument, KdlNode, KdlValue};
use std::fs;
use std::path::{Path, PathBuf};

pub fn nout_dir() -> PathBuf {
    PathBuf::from("nout")
}

pub fn packs_dir() -> PathBuf {
    nout_dir().join("packs")
}

pub fn build_dir() -> PathBuf {
    nout_dir().join("build")
}

pub const GITIGNORE_CONTENT: &str = "# Nibble\nnout/\n";

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct AtMetadata {
    pub name: String,
    pub entry: Option<String>,
    pub version: String,
    pub output: Option<String>,
    pub description: Option<String>,
    pub authors: Vec<String>,
}

impl AtMetadata {
    pub fn output_name(&self) -> String {
        self.output.clone().unwrap_or_else(|| self.name.clone())
    }
}

#[derive(Debug, Clone)]
pub struct Dependency {
    pub alias: String,
    pub at: String,
    pub version: Option<String>,
    pub source: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Manifest {
    pub at: AtMetadata,
    pub dependencies: Vec<Dependency>,
}

fn kdl_str(value: &KdlValue) -> Option<String> {
    value.as_string().map(|s| s.to_string())
}

fn optional_str_prop(node: &KdlNode, key: &str) -> Result<Option<String>> {
    for entry in node.entries() {
        if let Some(name) = entry.name()
            && name.value() == key
        {
            return kdl_str(entry.value()).map(Some).ok_or_else(|| {
                anyhow!(
                    "'{}' on node '{}' must be a string",
                    key,
                    node.name().value()
                )
            });
        }
    }

    Ok(None)
}

fn require_str_prop(node: &KdlNode, key: &str) -> Result<String> {
    optional_str_prop(node, key)?.ok_or_else(|| {
        anyhow!(
            "Missing required '{}' attribute on node '{}'",
            key,
            node.name().value()
        )
    })
}

fn ensure_known_props(node: &KdlNode, allowed: &[&str], allow_positional: bool) -> Result<()> {
    for entry in node.entries() {
        match entry.name() {
            Some(name) => {
                if !allowed.contains(&name.value()) {
                    return Err(anyhow!(
                        "Unknown attribute '{}' on node '{}' (expected one of: {})",
                        name.value(),
                        node.name().value(),
                        allowed.join(", ")
                    ));
                }
            }
            None => {
                if !allow_positional {
                    return Err(anyhow!(
                        "Unexpected positional value on node '{}'",
                        node.name().value()
                    ));
                }
            }
        }
    }

    Ok(())
}

fn parse_at_node(node: &KdlNode) -> Result<AtMetadata> {
    const ALLOWED: &[&str] = &["name", "entry", "version", "output", "description"];
    ensure_known_props(node, ALLOWED, false)?;

    let name = require_str_prop(node, "name")?;
    let entry = optional_str_prop(node, "entry")?;
    let version = require_str_prop(node, "version")?;
    let output = optional_str_prop(node, "output")?;
    let description = optional_str_prop(node, "description")?;

    let mut authors = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            match child.name().value() {
                "author" => {
                    ensure_known_props(child, &[], true)?;

                    let mut entries = child.entries().iter();

                    let value = entries.next().ok_or_else(|| {
                        anyhow!("'author' node requires a name, e.g. author \"Someone\"")
                    })?;

                    if value.name().is_some() {
                        return Err(anyhow!(
                            "'author' expects a positional value, not a named attribute"
                        ));
                    }

                    let author_name = kdl_str(value.value())
                        .ok_or_else(|| anyhow!("'author' value must be a string"))?;

                    authors.push(author_name);
                }

                other => {
                    return Err(anyhow!(
                        "Unknown child node '{}' under 'at' (expected 'author')",
                        other
                    ));
                }
            }
        }
    }

    Ok(AtMetadata {
        name,
        entry,
        version,
        output,
        description,
        authors,
    })
}

fn parse_dependency_node(node: &KdlNode) -> Result<Dependency> {
    const ALLOWED: &[&str] = &["at", "version", "source"];
    ensure_known_props(node, ALLOWED, false)?;

    if node.children().is_some() {
        return Err(anyhow!(
            "@{} has unexpected child nodes; dependencies are attribute-only",
            node.name().value()
        ));
    }

    let alias = node.name().value().to_string();
    let at = optional_str_prop(node, "at")?.unwrap_or_else(|| alias.clone());
    let version = optional_str_prop(node, "version")?;
    let source = optional_str_prop(node, "source")?;

    Ok(Dependency {
        alias,
        at,
        version,
        source,
    })
}

fn parse_dependencies_node(node: &KdlNode) -> Result<Vec<Dependency>> {
    ensure_known_props(node, &[], false)?;

    let mut deps = Vec::new();

    if let Some(children) = node.children() {
        for child in children.nodes() {
            deps.push(parse_dependency_node(child)?);
        }
    }

    Ok(deps)
}

pub fn parse_manifest(text: &str) -> Result<Manifest> {
    let doc: KdlDocument = text
        .parse()
        .map_err(|e| anyhow!("Failed to parse manifest.nibble: {}", e))?;

    let mut at: Option<AtMetadata> = None;
    let mut dependencies = Vec::new();

    for node in doc.nodes() {
        match node.name().value() {
            "at" => {
                if at.is_some() {
                    return Err(anyhow!(
                        "Duplicate 'at' node in manifest.nibble"
                    ));
                }

                at = Some(parse_at_node(node)?);
            }

            "dependencies" => {
                dependencies = parse_dependencies_node(node)?;
            }

            other => {
                return Err(anyhow!(
                    "Unknown top-level node '{}' in manifest.nibble \
                     (expected 'at', 'compiler', or 'dependencies')",
                    other
                ));
            }
        }
    }

    let at = at.ok_or_else(|| {
        anyhow!(
            "manifest.nibble is missing a required 'at' node"
        )
    })?;

    Ok(Manifest {
        at,
        dependencies,
    })
}

pub fn manifest_path() -> PathBuf {
    PathBuf::from("manifest.nibble")
}

pub fn load_manifest() -> Result<Option<Manifest>> {
    let path = manifest_path();

    if !path.exists() {
        return Ok(None);
    }

    let content =
        fs::read_to_string(&path)
            .with_context(|| format!("Failed to read {:?}", path))?;

    let manifest =
        parse_manifest(&content)
            .with_context(|| format!("Invalid manifest at {:?}", path))?;

    Ok(Some(manifest))
}

pub fn require_manifest() -> Result<Manifest> {
    load_manifest()?.ok_or_else(|| {
        anyhow!(
            "No manifest.nibble found in the current directory. \
             Run `nibble init` to create a new project."
        )
    })
}

pub fn add_dependency_to_manifest(dep: &Dependency) -> Result<()> {
    let path = manifest_path();

    let content = fs::read_to_string(&path)
        .with_context(|| {
            format!(
                "Failed to read {:?} — run `nibble init` first?",
                path
            )
        })?;

    let mut doc: KdlDocument = content
        .parse()
        .map_err(|e| anyhow!("Failed to parse manifest.nibble: {}", e))?;

    parse_manifest(&content)?;

    let mut new_dep_node = KdlNode::new(dep.alias.clone());

    if dep.at != dep.alias {
        new_dep_node.push(("at", dep.at.clone()));
    }

    if let Some(version) = &dep.version {
        new_dep_node.push(("version", version.clone()));
    }

    if let Some(source) = &dep.source {
        new_dep_node.push(("source", source.clone()));
    }

    let deps_node_idx = doc
        .nodes()
        .iter()
        .position(|n| n.name().value() == "dependencies");

    match deps_node_idx {
        Some(idx) => {
            let deps_node = &mut doc.nodes_mut()[idx];

            let children = deps_node
                .children_mut()
                .get_or_insert_with(KdlDocument::new);

            if let Some(existing_idx) = children
                .nodes()
                .iter()
                .position(|n| n.name().value() == dep.alias)
            {
                children.nodes_mut()[existing_idx] = new_dep_node;
            } else {
                children.nodes_mut().push(new_dep_node);
            }
        }

        None => {
            let mut deps_node = KdlNode::new("dependencies");
            let mut children = KdlDocument::new();

            children.nodes_mut().push(new_dep_node);
            deps_node.set_children(children);

            doc.nodes_mut().push(deps_node);
        }
    }

    fs::write(&path, doc.to_string())
        .with_context(|| format!("Failed to write {:?}", path))?;

    Ok(())
}

/// Resolve a registry AT name to its repository.
///
/// Every fetched AT is expected to carry its own manifest.nibble
/// describing its own layout.
fn default_registry() -> std::collections::HashMap<&'static str, &'static str> {
    let mut registry = std::collections::HashMap::new();

    registry.insert("std", "mysz-lang/mysz-std");

    registry
}

fn source_to_tarball_url(source: &str, version: Option<&str>) -> Result<String> {
    if let Some(repo) = source.strip_prefix("github:") {
        let repo = repo.trim_matches('/');

        let mut parts = repo.split('/');
        let owner = parts.next();
        let name = parts.next();

        if owner.is_none() || name.is_none() || parts.next().is_some() {
            return Err(anyhow!(
                "Invalid GitHub source '{}': expected github:owner/repo",
                source
            ));
        }

        return Ok(match version {
            Some(version) => format!(
                "https://github.com/{}/archive/refs/tags/{}.tar.gz",
                repo, version
            ),

            None => format!(
                "https://github.com/{}/archive/refs/heads/main.tar.gz",
                repo
            ),
        });
    }

    if let Some(repo) = source.strip_prefix("gitlab:") {
        let repo = repo.trim_matches('/');

        let mut parts = repo.split('/');
        let owner = parts.next();
        let name = parts.next();

        if owner.is_none() || name.is_none() || parts.next().is_some() {
            return Err(anyhow!(
                "Invalid GitLab source '{}': expected gitlab:owner/repo",
                source
            ));
        }

        let name = name.unwrap();

        return Ok(match version {
            Some(version) => format!(
                "https://gitlab.com/{}/-/archive/{}/{}-{}.tar.gz",
                repo, version, name, version
            ),

            None => format!(
                "https://gitlab.com/{}/-/archive/main/{}-main.tar.gz",
                repo, name
            ),
        });
    }

    let version = version.unwrap_or("main");
    Ok(source.replace("{version}", version))
}

fn resolve_tarball_url(dep: &Dependency) -> Result<String> {
    if let Some(source) = &dep.source {
        return source_to_tarball_url(source, dep.version.as_deref());
    }

    let registry = default_registry();

    let repo = registry.get(dep.at.as_str()).ok_or_else(|| {
        anyhow!(
            "@{} is not in the default registry and has no explicit 'source'",
            dep.at
        )
    })?;

    match &dep.version {
        Some(version) => Ok(format!(
            "https://github.com/{}/archive/refs/tags/{}.tar.gz",
            repo, version
        )),

        None => Ok(format!(
            "https://github.com/{}/archive/refs/heads/main.tar.gz",
            repo
        )),
    }
}

pub fn install_dependency(dep: &Dependency) -> Result<()> {
    let target_dir = packs_dir().join(&dep.alias);

    if target_dir.exists() && fs::read_dir(&target_dir)?.next().is_some() {
        return Ok(());
    }

    let url = resolve_tarball_url(dep)?;

    println!(
        "\x1b[1;36mDownloading\x1b[0m @{} ({})...",
        dep.alias, dep.at
    );

    let response = reqwest::blocking::get(&url).with_context(|| {
        format!(
            "Network connection failed while fetching @{}. \
             Check your internet access.",
            dep.alias
        )
    })?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch @{}: server returned status {}",
            dep.alias,
            response.status()
        ));
    }

    let tar_gz = flate2::read::GzDecoder::new(response);
    let mut archive = tar::Archive::new(tar_gz);

    fs::create_dir_all(&target_dir)
        .with_context(|| {
            format!(
                "Failed to create cache directory for @{} at {:?}",
                dep.alias, target_dir
            )
        })?;

    let mut extracted_count = 0;

    for entry_result in archive.entries().context(
        "Failed to read dependency archive entries"
    )? {
        let mut entry = entry_result
            .context("Corrupt entry in downloaded dependency archive")?;

        let path = entry
            .path()
            .context("Dependency archive entry has no path")?
            .to_path_buf();

        let mut components = path.components();

        if components.next().is_none() {
            continue;
        }

        let rest: PathBuf = components.collect();

        if rest.as_os_str().is_empty() {
            continue;
        }

        let out_path = target_dir.join(&rest);

        if let Some(parent) = out_path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| {
                    format!(
                        "Failed to create directory while extracting @{}",
                        dep.alias
                    )
                })?;
        }

        if entry.header().entry_type().is_file() {
            entry.unpack(&out_path)
                .with_context(|| {
                    format!(
                        "Failed to extract {:?} from @{}",
                        out_path, dep.alias
                    )
                })?;

            extracted_count += 1;
        }
    }

    if extracted_count == 0 {
        return Err(anyhow!(
            "Downloaded archive for @{} contained no files",
            dep.alias
        ));
    }

    verify_fetched_manifest(dep, &target_dir)?;

    println!(
        "\x1b[1;32mInstalled\x1b[0m @{} successfully ({} files extracted).",
        dep.alias, extracted_count
    );

    Ok(())
}

fn verify_fetched_manifest(
    dep: &Dependency,
    pkg_dir: &Path,
) -> Result<()> {
    let manifest_file = pkg_dir.join("manifest.nibble");

    let content = fs::read_to_string(&manifest_file)
        .with_context(|| {
            format!(
                "Fetched @{} has no manifest.nibble at its root ({:?})",
                dep.alias, manifest_file
            )
        })?;

    let fetched = parse_manifest(&content)
        .with_context(|| {
            format!(
                "@{} has an invalid manifest.nibble",
                dep.alias
            )
        })?;

    if fetched.at.name != dep.at {
        return Err(anyhow!(
            "@{} was expected to be @{}, but its manifest declares @{}",
            dep.alias,
            dep.at,
            fetched.at.name
        ));
    }

    if let Some(requested_version) = &dep.version
        && &fetched.at.version != requested_version
    {
        return Err(anyhow!(
            "@{} version mismatch: requested '{}', \
             fetched @{} declares '{}'",
            dep.alias,
            requested_version,
            fetched.at.name,
            fetched.at.version
        ));
    }

    Ok(())
}

pub fn resolve_local_manifest() -> Result<()> {
    let Some(manifest) = load_manifest()? else {
        return Ok(());
    };

    for dep in &manifest.dependencies {
        install_dependency(dep)?;
    }

    Ok(())
}

pub fn list() -> Result<()> {
    let manifest = require_manifest()?;
    let packs_dir = packs_dir();

    // The project's main AT is always listed first.
    println!("@{}", manifest.at.name);

    for dep in &manifest.dependencies {
        let cached = packs_dir
            .join(&dep.alias)
            .join("manifest.nibble")
            .is_file();

        if cached {
            println!("@{}", dep.alias);
        } else {
            println!("@{} (not cached)", dep.alias);
        }
    }

    Ok(())
}

pub fn clean() -> Result<()> {
    let dir = nout_dir();

    if !dir.exists() {
        return Ok(());
    }

    fs::remove_dir_all(&dir)
        .with_context(|| format!("Failed to remove {:?}", dir))?;

    println!(
        "\x1b[1;32mCleaned\x1b[0m {}",
        dir.display()
    );

    Ok(())
}