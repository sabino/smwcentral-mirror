use anyhow::{bail, Context, Result};
use sha1::{Digest, Sha1};
use std::fs;
use std::path::Path;

pub const SMW_USA_UNHEADERED_SIZE: u64 = 524_288;
pub const SMW_USA_UNHEADERED_SHA1: &str = "6b47bb75d16514b6a476aa0c73a683a2a4c18765";

#[derive(Debug, Clone)]
pub struct RomInfo {
    pub path: String,
    pub size: u64,
    pub sha1: String,
    pub valid_unheadered_usa: bool,
}

pub fn inspect_rom(path: &Path) -> Result<RomInfo> {
    let bytes = fs::read(path).with_context(|| format!("reading ROM {}", path.display()))?;
    let size = bytes.len() as u64;
    let sha1 = hex::encode(Sha1::digest(&bytes));
    Ok(RomInfo {
        path: path.display().to_string(),
        size,
        valid_unheadered_usa: size == SMW_USA_UNHEADERED_SIZE && sha1 == SMW_USA_UNHEADERED_SHA1,
        sha1,
    })
}

pub fn verify_rom(path: &Path) -> Result<RomInfo> {
    let info = inspect_rom(path)?;
    if !info.valid_unheadered_usa {
        bail!(
            "{} is not the verified unheadered SMW USA ROM: got {} bytes sha1 {}",
            path.display(),
            info.size,
            info.sha1
        );
    }
    Ok(info)
}
