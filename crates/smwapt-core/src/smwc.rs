use crate::package::{canonical_package_name, known_aliases, InstallKind, Package, PackageVersion};
use anyhow::{Context, Result};
use reqwest::blocking::Client;
use reqwest::StatusCode;
use serde::Deserialize;
use std::thread;
use std::time::Duration;

const BASE_URL: &str = "https://www.smwcentral.net/ajax.php";

#[derive(Debug, Clone)]
pub struct SyncOptions {
    pub sections: Vec<String>,
    pub max_pages: Option<u64>,
    pub delay_ms: u64,
}

impl Default for SyncOptions {
    fn default() -> Self {
        Self {
            sections: vec![
                "tools".to_string(),
                "smwpatches".to_string(),
                "uberasm".to_string(),
                "smwblocks".to_string(),
                "smwsprites".to_string(),
                "smwmusic".to_string(),
                "smwgraphics".to_string(),
            ],
            max_pages: Some(1),
            delay_ms: 1_150,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Page {
    current_page: u64,
    last_page: u64,
    data: Vec<FileRecord>,
}

#[derive(Debug, Deserialize)]
struct FileRecord {
    id: u64,
    section: String,
    name: String,
    time: i64,
    authors: Vec<UserRecord>,
    tags: Option<Vec<String>>,
    size: u64,
    download_url: String,
    fields: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
struct UserRecord {
    name: String,
}

pub fn sync_packages(options: &SyncOptions) -> Result<Vec<Package>> {
    let client = Client::builder()
        .user_agent("smwapt/0.1 (+https://github.com/smwapt/smwapt)")
        .timeout(Duration::from_secs(20))
        .build()?;
    let mut packages = Vec::new();
    for section in &options.sections {
        let mut page_no = 1;
        loop {
            let page = fetch_page(&client, section, page_no)
                .with_context(|| format!("syncing SMW Central section {section} page {page_no}"))?;
            for record in page.data {
                packages.push(record_to_package(record));
            }
            let last = options
                .max_pages
                .map(|max| max.min(page.last_page))
                .unwrap_or(page.last_page);
            if page.current_page >= last {
                break;
            }
            page_no += 1;
            thread::sleep(Duration::from_millis(options.delay_ms));
        }
        thread::sleep(Duration::from_millis(options.delay_ms));
    }
    Ok(merge_packages(packages))
}

fn fetch_page(client: &Client, section: &str, page: u64) -> Result<Page> {
    let mut wait = Duration::from_secs(10);
    for attempt in 1..=6 {
        let mut req =
            client
                .get(BASE_URL)
                .query(&[("a", "getsectionlist"), ("s", section), ("u", "0")]);
        let page_s;
        if page > 1 {
            page_s = page.to_string();
            req = req.query(&[("n", page_s.as_str())]);
        }
        let response = req.send()?;
        if response.status() == StatusCode::TOO_MANY_REQUESTS {
            let retry_after = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| value.parse::<u64>().ok())
                .map(Duration::from_secs)
                .unwrap_or(wait);
            if attempt == 6 {
                response.error_for_status()?;
            }
            thread::sleep(retry_after);
            wait = (wait * 2).min(Duration::from_secs(120));
            continue;
        }
        let response = response.error_for_status()?;
        return Ok(response.json()?);
    }
    unreachable!("retry loop either returns or errors")
}

fn record_to_package(record: FileRecord) -> Package {
    let install_kind = InstallKind::from_section(&record.section);
    let version = version_from_record(&record);
    let aliases = known_aliases(&record.section, record.id, &record.name);
    let name = aliases
        .first()
        .cloned()
        .unwrap_or_else(|| canonical_package_name(&record.section, &record.name, &version));
    let filename = record
        .download_url
        .rsplit('/')
        .next()
        .map(urlencoding_like_decode)
        .unwrap_or_else(|| format!("{}.zip", name));
    let description = record
        .fields
        .as_ref()
        .and_then(|fields| fields.get("description"))
        .and_then(|value| value.as_str())
        .unwrap_or("")
        .to_string();
    let title = record.name;
    Package {
        name,
        aliases,
        section: record.section.clone(),
        upstream_id: record.id,
        title: title.clone(),
        authors: record
            .authors
            .into_iter()
            .map(|author| author.name)
            .collect(),
        tags: record.tags.unwrap_or_default(),
        description,
        latest_version: version.clone(),
        install_kind,
        versions: vec![PackageVersion {
            upstream_id: record.id,
            title,
            version,
            upstream_time: record.time,
            download_url: record.download_url,
            filename,
            size: record.size,
            sha256: None,
            dependencies: dependencies_for(install_kind),
            install_kind,
        }],
    }
}

fn merge_packages(mut packages: Vec<Package>) -> Vec<Package> {
    packages.sort_by(|a, b| {
        a.name.cmp(&b.name).then(
            b.versions[0]
                .upstream_time
                .cmp(&a.versions[0].upstream_time),
        )
    });
    let mut merged: Vec<Package> = Vec::new();
    for package in packages {
        if let Some(existing) = merged
            .iter_mut()
            .find(|candidate| candidate.name == package.name)
        {
            existing.aliases.extend(package.aliases);
            existing.tags.extend(package.tags);
            existing.versions.extend(package.versions);
            existing.versions.sort_by(|a, b| {
                version_sort_key(&b.version)
                    .cmp(&version_sort_key(&a.version))
                    .then(b.upstream_time.cmp(&a.upstream_time))
            });
            existing.aliases.sort();
            existing.aliases.dedup();
            existing.tags.sort();
            existing.tags.dedup();
            if let Some(latest) = existing.versions.first() {
                existing.latest_version = latest.version.clone();
                existing.upstream_id = latest.upstream_id;
                existing.title = latest.title.clone();
            }
        } else {
            merged.push(package);
        }
    }
    merged
}

fn version_sort_key(version: &str) -> Vec<u64> {
    version
        .split(|ch: char| !ch.is_ascii_digit())
        .filter(|part| !part.is_empty())
        .map(|part| part.parse::<u64>().unwrap_or(0))
        .collect()
}

fn version_from_record(record: &FileRecord) -> String {
    if let Some(version) = record
        .fields
        .as_ref()
        .and_then(|fields| fields.get("version"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        return version.trim().to_string();
    }
    format!("smwc-{}-{}", record.id, record.time)
}

fn dependencies_for(kind: InstallKind) -> Vec<String> {
    match kind {
        InstallKind::AsarPatch => vec!["asar".to_string()],
        InstallKind::UberAsm => vec!["uberasm-tool".to_string()],
        InstallKind::GpsBlock => vec!["gps".to_string()],
        InstallKind::PixiSprite => vec!["pixi".to_string()],
        InstallKind::AddMusicKMusic => vec!["addmusick".to_string()],
        _ => Vec::new(),
    }
}

fn urlencoding_like_decode(input: &str) -> String {
    input
        .replace("%20", " ")
        .replace("%28", "(")
        .replace("%29", ")")
        .replace("%2B", "+")
}
