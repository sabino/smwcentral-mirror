use crate::package::{Package, RegistryIndex};
use crate::sources::{source_cache_key, Source};
use anyhow::{Context, Result};
use chrono::Utc;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::blocking::Client;
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

pub const SUITE: &str = "stable";
pub const COMPONENT: &str = "main";
pub const ARCH: &str = "binary-smw";

pub fn write_repository(repo_dir: &Path, packages: Vec<Package>) -> Result<RegistryIndex> {
    let index = RegistryIndex {
        generated_at: Utc::now().to_rfc3339(),
        suite: SUITE.to_string(),
        component: COMPONENT.to_string(),
        packages,
    };
    fs::create_dir_all(repo_dir)?;
    fs::write(
        repo_dir.join("index.json"),
        serde_json::to_string_pretty(&index)?,
    )?;
    let dist_dir = repo_dir.join(format!("dists/{SUITE}/{COMPONENT}/{ARCH}"));
    fs::create_dir_all(&dist_dir)?;
    let packages_text = render_packages(&index.packages);
    fs::write(dist_dir.join("Packages"), &packages_text)?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(packages_text.as_bytes())?;
    fs::write(dist_dir.join("Packages.gz"), gz.finish()?)?;
    fs::write(
        repo_dir.join(format!("dists/{SUITE}/Release")),
        render_release(),
    )?;
    Ok(index)
}

pub fn load_repository(repo_dir: &Path) -> Result<RegistryIndex> {
    let path = repo_dir.join("index.json");
    let content =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

pub fn validate_repository(repo_dir: &Path) -> Result<RegistryIndex> {
    let index = load_repository(repo_dir)?;
    let packages_path = repo_dir.join(format!("dists/{SUITE}/{COMPONENT}/{ARCH}/Packages"));
    let packages_gz_path = packages_path.with_file_name("Packages.gz");
    let release_path = repo_dir.join(format!("dists/{SUITE}/Release"));
    fs::read_to_string(&packages_path)
        .with_context(|| format!("reading {}", packages_path.display()))?;
    let gz = fs::File::open(&packages_gz_path)
        .with_context(|| format!("reading {}", packages_gz_path.display()))?;
    let mut decoder = GzDecoder::new(gz);
    let mut decoded = String::new();
    decoder
        .read_to_string(&mut decoded)
        .with_context(|| format!("decoding {}", packages_gz_path.display()))?;
    if decoded.is_empty() && !index.packages.is_empty() {
        anyhow::bail!(
            "{} decoded to an empty Packages file",
            packages_gz_path.display()
        );
    }
    fs::read_to_string(&release_path)
        .with_context(|| format!("reading {}", release_path.display()))?;
    Ok(index)
}

pub fn render_packages(packages: &[Package]) -> String {
    let mut out = String::new();
    for pkg in packages {
        let version = &pkg.versions[0];
        out.push_str(&format!("Package: {}\n", pkg.name));
        out.push_str(&format!("Version: {}\n", version.version));
        out.push_str(&format!("Section: {}\n", pkg.section));
        out.push_str("Architecture: smw\n");
        out.push_str(&format!("Maintainer: {}\n", pkg.authors.join(", ")));
        out.push_str(&format!(
            "Filename: pool/{}/{}/{}\n",
            pkg.section, pkg.upstream_id, version.filename
        ));
        out.push_str(&format!("Size: {}\n", version.size));
        out.push_str(&format!("X-SMWC-ID: {}\n", pkg.upstream_id));
        out.push_str(&format!("X-SMWAPT-Kind: {:?}\n", pkg.install_kind));
        out.push_str(&format!("X-SMWAPT-URL: {}\n", version.download_url));
        out.push_str(&format!("Description: {}\n\n", one_line(&pkg.title)));
    }
    out
}

fn render_release() -> String {
    format!(
        "Origin: smwapt\nLabel: smwapt\nSuite: {SUITE}\nCodename: {SUITE}\nArchitectures: smw\nComponents: {COMPONENT}\nDescription: SMW package registry\n"
    )
}

fn one_line(input: &str) -> String {
    input.split_whitespace().collect::<Vec<_>>().join(" ")
}

pub fn cache_packages_from_sources(sources: &[Source], cache_dir: &Path) -> Result<Vec<Package>> {
    fs::create_dir_all(cache_dir)?;
    let client = Client::builder()
        .user_agent("smwapt/0.1")
        .timeout(std::time::Duration::from_secs(20))
        .build()?;
    let mut all = Vec::new();
    for source in sources {
        let packages = fetch_source_packages(&client, source)?;
        let path = cache_path_for_source(cache_dir, source);
        fs::write(&path, serde_json::to_string_pretty(&packages)?)?;
        all.extend(packages);
    }
    all.sort_by(|a, b| a.name.cmp(&b.name));
    fs::write(
        cache_dir.join("packages.json"),
        serde_json::to_string_pretty(&all)?,
    )?;
    Ok(all)
}

fn fetch_source_packages(client: &Client, source: &Source) -> Result<Vec<Package>> {
    let api_url = format!("{}/api/v1/packages", source.url);
    match fetch_live_packages(client, &api_url) {
        Ok(packages) => return Ok(packages),
        Err(api_error) => {
            let index_url = format!("{}/index.json", source.url);
            fetch_static_index(client, source, &index_url).with_context(|| {
                format!(
                    "source {} was neither a smwapt server API nor a static repository; API error: {api_error:#}",
                    source.url
                )
            })
        }
    }
}

fn fetch_live_packages(client: &Client, url: &str) -> Result<Vec<Package>> {
    client
        .get(url)
        .send()
        .with_context(|| format!("fetching {url}"))?
        .error_for_status()
        .with_context(|| format!("fetching {url}"))?
        .json()
        .with_context(|| format!("parsing {url}"))
}

fn fetch_static_index(client: &Client, source: &Source, url: &str) -> Result<Vec<Package>> {
    let index: RegistryIndex = client
        .get(url)
        .send()
        .with_context(|| format!("fetching {url}"))?
        .error_for_status()
        .with_context(|| format!("fetching {url}"))?
        .json()
        .with_context(|| format!("parsing {url}"))?;
    if index.suite != source.suite {
        anyhow::bail!(
            "source suite mismatch: source requested {}, repository has {}",
            source.suite,
            index.suite
        );
    }
    if index.component != source.component {
        anyhow::bail!(
            "source component mismatch: source requested {}, repository has {}",
            source.component,
            index.component
        );
    }
    Ok(index.packages)
}

pub fn load_cached_packages(cache_dir: &Path) -> Result<Vec<Package>> {
    let path = cache_dir.join("packages.json");
    if !path.exists() {
        return Ok(Vec::new());
    }
    let content =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

fn cache_path_for_source(cache_dir: &Path, source: &Source) -> PathBuf {
    cache_dir.join(format!("{}.json", source_cache_key(source)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::package::{InstallKind, PackageVersion};

    #[test]
    fn renders_apt_package_stanza() {
        let packages = vec![Package {
            name: "asar".to_string(),
            aliases: vec!["asar".to_string()],
            section: "tools".to_string(),
            upstream_id: 37443,
            title: "Asar v1.91".to_string(),
            authors: vec!["RPG Hacker".to_string()],
            tags: vec![],
            description: String::new(),
            latest_version: "smwc-37443".to_string(),
            install_kind: InstallKind::Tool,
            versions: vec![PackageVersion {
                upstream_id: 37443,
                title: "Asar v1.91".to_string(),
                version: "smwc-37443".to_string(),
                upstream_time: 0,
                download_url: "https://example.invalid/asar.zip".to_string(),
                filename: "asar.zip".to_string(),
                size: 42,
                sha256: None,
                dependencies: vec![],
                install_kind: InstallKind::Tool,
            }],
        }];
        let rendered = render_packages(&packages);
        assert!(rendered.contains("Package: asar"));
        assert!(rendered.contains("Architecture: smw"));
        assert!(rendered.contains("X-SMWC-ID: 37443"));
    }

    #[test]
    fn validates_static_repository_tree() {
        let dir = tempfile::tempdir().unwrap();
        let packages = vec![Package {
            name: "asar".to_string(),
            aliases: vec!["asar".to_string()],
            section: "tools".to_string(),
            upstream_id: 37443,
            title: "Asar v1.91".to_string(),
            authors: vec!["RPG Hacker".to_string()],
            tags: vec![],
            description: String::new(),
            latest_version: "1.91".to_string(),
            install_kind: InstallKind::Tool,
            versions: vec![PackageVersion {
                upstream_id: 37443,
                title: "Asar v1.91".to_string(),
                version: "1.91".to_string(),
                upstream_time: 0,
                download_url: "https://example.invalid/asar.zip".to_string(),
                filename: "asar.zip".to_string(),
                size: 42,
                sha256: None,
                dependencies: vec![],
                install_kind: InstallKind::Tool,
            }],
        }];
        write_repository(dir.path(), packages).unwrap();
        let index = validate_repository(dir.path()).unwrap();
        assert_eq!(index.packages.len(), 1);
        assert_eq!(index.packages[0].name, "asar");
    }
}
