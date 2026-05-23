use crate::package::{known_aliases, normalize_name, InstallKind, Package, PackageVersion};
use anyhow::{Context, Result};
use reqwest::blocking::Client;
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
    packages.sort_by(|a, b| a.name.cmp(&b.name).then(a.upstream_id.cmp(&b.upstream_id)));
    Ok(packages)
}

fn fetch_page(client: &Client, section: &str, page: u64) -> Result<Page> {
    let mut req = client
        .get(BASE_URL)
        .query(&[("a", "getsectionlist"), ("s", section), ("u", "0")]);
    let page_s;
    if page > 1 {
        page_s = page.to_string();
        req = req.query(&[("n", page_s.as_str())]);
    }
    let response = req.send()?.error_for_status()?;
    Ok(response.json()?)
}

fn record_to_package(record: FileRecord) -> Package {
    let install_kind = InstallKind::from_section(&record.section);
    let aliases = known_aliases(&record.section, record.id, &record.name);
    let name = aliases
        .first()
        .cloned()
        .unwrap_or_else(|| format!("{}-{}", record.section, normalize_name(&record.name)));
    let version = version_from_record(&record);
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
    Package {
        name,
        aliases,
        section: record.section.clone(),
        upstream_id: record.id,
        title: record.name,
        authors: record.authors.into_iter().map(|author| author.name).collect(),
        tags: record.tags.unwrap_or_default(),
        description,
        latest_version: version.clone(),
        install_kind,
        versions: vec![PackageVersion {
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
    input.replace("%20", " ")
        .replace("%28", "(")
        .replace("%29", ")")
        .replace("%2B", "+")
}
