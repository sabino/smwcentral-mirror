use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallKind {
    Tool,
    AsarPatch,
    UberAsm,
    GpsBlock,
    PixiSprite,
    AddMusicKMusic,
    AssetOnly,
}

impl InstallKind {
    pub fn from_section(section: &str) -> Self {
        match section {
            "tools" => Self::Tool,
            "smwpatches" => Self::AsarPatch,
            "uberasm" => Self::UberAsm,
            "smwblocks" => Self::GpsBlock,
            "smwsprites" => Self::PixiSprite,
            "smwmusic" => Self::AddMusicKMusic,
            _ => Self::AssetOnly,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub aliases: Vec<String>,
    pub section: String,
    pub upstream_id: u64,
    pub title: String,
    pub authors: Vec<String>,
    pub tags: Vec<String>,
    pub description: String,
    pub latest_version: String,
    pub install_kind: InstallKind,
    pub versions: Vec<PackageVersion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageVersion {
    pub upstream_id: u64,
    pub title: String,
    pub version: String,
    pub upstream_time: i64,
    pub download_url: String,
    pub filename: String,
    pub size: u64,
    pub sha256: Option<String>,
    pub dependencies: Vec<String>,
    pub install_kind: InstallKind,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryIndex {
    pub generated_at: String,
    pub suite: String,
    pub component: String,
    pub packages: Vec<Package>,
}

impl RegistryIndex {
    pub fn find(&self, query: &str) -> Option<&Package> {
        self.packages
            .iter()
            .find(|pkg| pkg.name == query || pkg.aliases.iter().any(|alias| alias == query))
    }
}

pub fn normalize_name(input: &str) -> String {
    let mut out = String::new();
    let mut last_dash = false;
    for ch in input.chars().flat_map(|c| c.to_lowercase()) {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
            last_dash = false;
        } else if !last_dash {
            out.push('-');
            last_dash = true;
        }
    }
    out.trim_matches('-').to_string()
}

pub fn canonical_package_name(section: &str, title: &str, version: &str) -> String {
    format!("{section}-{}", canonical_title_slug(title, version))
}

pub fn canonical_title_slug(title: &str, version: &str) -> String {
    let normalized = normalize_name(title);
    let version_slug = normalize_name(version);
    let mut candidates = Vec::new();
    if !version_slug.is_empty() {
        candidates.push(format!("-v{version_slug}"));
        candidates.push(format!("-{version_slug}"));
    }
    candidates.push("-version".to_string());
    for suffix in candidates {
        if let Some(stripped) = normalized.strip_suffix(&suffix) {
            return stripped.trim_matches('-').to_string();
        }
    }
    strip_generic_version_suffix(&normalized)
}

fn strip_generic_version_suffix(slug: &str) -> String {
    let parts: Vec<&str> = slug.split('-').collect();
    let Some(start) = parts.iter().rposition(|part| is_version_start_token(part)) else {
        return slug.to_string();
    };
    if start == 0 {
        return slug.to_string();
    }
    if parts[start..].iter().all(|part| is_version_token(part)) {
        return parts[..start].join("-");
    }
    slug.to_string()
}

fn is_version_start_token(token: &str) -> bool {
    if let Some(rest) = token.strip_prefix('v') {
        return !rest.is_empty() && rest.chars().all(|ch| ch.is_ascii_digit());
    }
    token.chars().all(|ch| ch.is_ascii_digit())
}

fn is_version_token(token: &str) -> bool {
    token
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || ch == 'v')
}

pub fn known_aliases(section: &str, id: u64, title: &str) -> Vec<String> {
    let normalized = normalize_name(title);
    let mut aliases = Vec::new();
    match id {
        37443 => aliases.push("asar".to_string()),
        39036 => aliases.push("uberasm-tool".to_string()),
        40056 => aliases.push("gps".to_string()),
        37432 => aliases.push("pixi".to_string()),
        37906 => aliases.push("addmusick".to_string()),
        42149 => aliases.push("flips".to_string()),
        41329 => aliases.push("lunar-magic".to_string()),
        38381 => aliases.push("snes9x".to_string()),
        _ => {}
    }
    if section == "tools" {
        if normalized.starts_with("asar") && !aliases.iter().any(|a| a == "asar") {
            aliases.push("asar".to_string());
        }
        if normalized.starts_with("uberasm-tool") {
            aliases.push("uberasm-tool".to_string());
        }
    }
    aliases.sort();
    aliases.dedup();
    aliases
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_package_names() {
        assert_eq!(normalize_name("UberASM Tool 2.1"), "uberasm-tool-2-1");
        assert_eq!(
            normalize_name("MessageBox in Minimalist Status Bar + Goal"),
            "messagebox-in-minimalist-status-bar-goal"
        );
    }

    #[test]
    fn canonical_names_strip_trailing_versions() {
        assert_eq!(
            canonical_package_name("uberasm", "Retry System v2.0.3", "2.0.3"),
            "uberasm-retry-system"
        );
        assert_eq!(
            canonical_package_name("tools", "UberASM Tool 2.1", "2.1"),
            "tools-uberasm-tool"
        );
        assert_eq!(
            canonical_package_name("smwpatches", "Retry System", "smwc-123-456"),
            "smwpatches-retry-system"
        );
    }

    #[test]
    fn maps_known_tool_aliases() {
        assert!(known_aliases("tools", 37443, "Asar v1.91").contains(&"asar".to_string()));
        assert!(
            known_aliases("tools", 39036, "UberASM Tool 2.1").contains(&"uberasm-tool".to_string())
        );
    }
}
