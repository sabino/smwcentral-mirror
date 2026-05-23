use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::fs;
use std::path::Path;
use url::Url;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Source {
    pub url: String,
    pub suite: String,
    pub component: String,
}

impl Source {
    pub fn parse(line: &str) -> Result<Self> {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            bail!("empty source line");
        }
        let parts: Vec<_> = trimmed.split_whitespace().collect();
        if parts.len() != 4 || parts[0] != "deb" {
            bail!("expected: deb <url> <suite> <component>");
        }
        Url::parse(parts[1]).with_context(|| format!("invalid source url {}", parts[1]))?;
        Ok(Self {
            url: parts[1].trim_end_matches('/').to_string(),
            suite: parts[2].to_string(),
            component: parts[3].to_string(),
        })
    }
}

impl fmt::Display for Source {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "deb {} {} {}", self.url, self.suite, self.component)
    }
}

pub fn read_sources(path: &Path) -> Result<Vec<Source>> {
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content = fs::read_to_string(path)
        .with_context(|| format!("reading source list {}", path.display()))?;
    let mut sources = Vec::new();
    for (idx, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        sources.push(Source::parse(trimmed).with_context(|| format!("line {}", idx + 1))?);
    }
    Ok(sources)
}

pub fn write_sources(path: &Path, sources: &[Source]) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut content = String::new();
    for source in sources {
        content.push_str(&source.to_string());
        content.push('\n');
    }
    fs::write(path, content).with_context(|| format!("writing {}", path.display()))
}

pub fn source_cache_key(source: &Source) -> String {
    crate::package::normalize_name(&format!(
        "{}-{}-{}",
        source.url, source.suite, source.component
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_apt_style_source() {
        let source = Source::parse("deb http://127.0.0.1:4789 stable main").unwrap();
        assert_eq!(source.url, "http://127.0.0.1:4789");
        assert_eq!(source.suite, "stable");
        assert_eq!(source.component, "main");
        assert_eq!(source.to_string(), "deb http://127.0.0.1:4789 stable main");
    }

    #[test]
    fn rejects_non_deb_source() {
        assert!(Source::parse("rpm http://127.0.0.1 stable main").is_err());
    }
}
