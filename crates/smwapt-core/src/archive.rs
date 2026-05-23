use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct ArchiveEntry {
    pub path: String,
    pub size: u64,
}

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).with_context(|| format!("opening {}", path.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = [0u8; 64 * 1024];
    loop {
        let read = file.read(&mut buf)?;
        if read == 0 {
            break;
        }
        hasher.update(&buf[..read]);
    }
    Ok(hex::encode(hasher.finalize()))
}

pub fn list_zip(path: &Path) -> Result<Vec<ArchiveEntry>> {
    let file = File::open(path).with_context(|| format!("opening archive {}", path.display()))?;
    let mut zip =
        ZipArchive::new(file).with_context(|| format!("reading zip {}", path.display()))?;
    let mut entries = Vec::new();
    for i in 0..zip.len() {
        let entry = zip.by_index(i)?;
        if !entry.is_dir() {
            entries.push(ArchiveEntry {
                path: entry.name().to_string(),
                size: entry.size(),
            });
        }
    }
    Ok(entries)
}

pub fn extract_zip(path: &Path, dest: &Path) -> Result<Vec<ArchiveEntry>> {
    fs::create_dir_all(dest).with_context(|| format!("creating {}", dest.display()))?;
    let file = File::open(path).with_context(|| format!("opening archive {}", path.display()))?;
    let mut zip =
        ZipArchive::new(file).with_context(|| format!("reading zip {}", path.display()))?;
    let mut entries = Vec::new();
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i)?;
        let enclosed = entry
            .enclosed_name()
            .map(PathBuf::from)
            .with_context(|| format!("unsafe zip path {}", entry.name()))?;
        let out = dest.join(enclosed);
        if entry.is_dir() {
            fs::create_dir_all(&out)?;
            continue;
        }
        if let Some(parent) = out.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut output = File::create(&out)?;
        io::copy(&mut entry, &mut output)?;
        entries.push(ArchiveEntry {
            path: entry.name().to_string(),
            size: entry.size(),
        });
    }
    Ok(entries)
}

pub fn choose_entry(
    entries: &[ArchiveEntry],
    requested: Option<&str>,
    extension: &str,
) -> Result<String> {
    if let Some(requested) = requested {
        if entries.iter().any(|entry| entry.path == requested) {
            return Ok(requested.to_string());
        }
        bail!("entry {requested} not found in archive");
    }
    let mut candidates: Vec<_> = entries
        .iter()
        .filter(|entry| entry.path.to_ascii_lowercase().ends_with(extension))
        .map(|entry| entry.path.clone())
        .collect();
    candidates.sort_by_key(|path| {
        let lower = path.to_ascii_lowercase();
        (
            lower.contains("readme") || lower.contains("sample") || lower.contains("template"),
            path.matches('/').count(),
            path.len(),
        )
    });
    candidates
        .into_iter()
        .next()
        .with_context(|| format!("archive has no {extension} file"))
}
