use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectManifest {
    pub rom: String,
    pub created_at: String,
    pub tool_dir: String,
    pub resource_dir: String,
}

impl ProjectManifest {
    pub fn new(rom: impl AsRef<Path>) -> Self {
        Self {
            rom: rom.as_ref().display().to_string(),
            created_at: Utc::now().to_rfc3339(),
            tool_dir: ".smwapt/tools".to_string(),
            resource_dir: "resources".to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Lockfile {
    pub installed: Vec<InstallRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstallRecord {
    pub name: String,
    pub version: String,
    pub upstream_id: u64,
    pub installed_at: String,
    pub install_kind: crate::package::InstallKind,
    pub archive_sha256: Option<String>,
    pub selected_entry: Option<String>,
    pub target: Option<String>,
    pub backup: Option<String>,
    pub command: Option<String>,
    pub status: String,
}

pub fn manifest_path(root: &Path) -> PathBuf {
    root.join(".smwapt/manifest.toml")
}

pub fn lockfile_path(root: &Path) -> PathBuf {
    root.join(".smwapt/lock.json")
}

pub fn read_manifest(root: &Path) -> Result<ProjectManifest> {
    let path = manifest_path(root);
    let content = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    toml::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

pub fn write_manifest(root: &Path, manifest: &ProjectManifest) -> Result<()> {
    let path = manifest_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, toml::to_string_pretty(manifest)?)
        .with_context(|| format!("writing {}", path.display()))
}

pub fn read_lockfile(root: &Path) -> Result<Lockfile> {
    let path = lockfile_path(root);
    if !path.exists() {
        return Ok(Lockfile::default());
    }
    let content = fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

pub fn write_lockfile(root: &Path, lockfile: &Lockfile) -> Result<()> {
    let path = lockfile_path(root);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(lockfile)?)
        .with_context(|| format!("writing {}", path.display()))
}
