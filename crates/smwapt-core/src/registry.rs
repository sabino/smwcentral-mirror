use crate::package::{Package, RegistryIndex};
use crate::sources::{source_cache_key, Source};
use anyhow::{Context, Result};
use chrono::Utc;
use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use reqwest::blocking::Client;
use sha1::{Digest as Sha1Digest, Sha1};
use sha2::Sha256;
use std::collections::BTreeMap;
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
    let legacy_index_path = repo_dir.join("index.json");
    if legacy_index_path.exists() {
        fs::remove_file(&legacy_index_path)
            .with_context(|| format!("removing {}", legacy_index_path.display()))?;
    }
    write_static_api(repo_dir, &index)?;
    write_homepage(repo_dir, &index)?;
    let dist_dir = repo_dir.join(format!("dists/{SUITE}/{COMPONENT}/{ARCH}"));
    fs::create_dir_all(&dist_dir)?;
    let packages_text = render_packages(&index.packages);
    let packages_bytes = packages_text.as_bytes();
    fs::write(dist_dir.join("Packages"), packages_bytes)?;
    let mut gz = GzEncoder::new(Vec::new(), Compression::default());
    gz.write_all(packages_bytes)?;
    let packages_gz = gz.finish()?;
    fs::write(dist_dir.join("Packages.gz"), &packages_gz)?;
    fs::write(
        repo_dir.join(format!("dists/{SUITE}/Release")),
        render_release(&[
            ReleaseFile::new(format!("{COMPONENT}/{ARCH}/Packages"), packages_bytes),
            ReleaseFile::new(format!("{COMPONENT}/{ARCH}/Packages.gz"), &packages_gz),
        ]),
    )?;
    Ok(index)
}

pub fn load_repository(repo_dir: &Path) -> Result<RegistryIndex> {
    let api_path = repo_dir.join("api/v1/index.json");
    let path = if api_path.exists() {
        api_path
    } else {
        repo_dir.join("index.json")
    };
    let content =
        fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    serde_json::from_str(&content).with_context(|| format!("parsing {}", path.display()))
}

pub fn validate_repository(repo_dir: &Path) -> Result<RegistryIndex> {
    let index = load_repository(repo_dir)?;
    let packages_path = repo_dir.join(format!("dists/{SUITE}/{COMPONENT}/{ARCH}/Packages"));
    let packages_gz_path = packages_path.with_file_name("Packages.gz");
    let release_path = repo_dir.join(format!("dists/{SUITE}/Release"));
    let homepage_path = repo_dir.join("index.html");
    let api_packages_path = repo_dir.join("api/v1/packages.json");
    let api_index_path = repo_dir.join("api/v1/index.json");
    let homepage = fs::read_to_string(&homepage_path)
        .with_context(|| format!("reading {}", homepage_path.display()))?;
    if !homepage.contains("data-smwapt-homepage") {
        anyhow::bail!("{} is not a smwapt homepage", homepage_path.display());
    }
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
    let api_packages: Vec<Package> = serde_json::from_str(
        &fs::read_to_string(&api_packages_path)
            .with_context(|| format!("reading {}", api_packages_path.display()))?,
    )
    .with_context(|| format!("parsing {}", api_packages_path.display()))?;
    if api_packages.len() != index.packages.len() {
        anyhow::bail!(
            "{} has {} packages, expected {}",
            api_packages_path.display(),
            api_packages.len(),
            index.packages.len()
        );
    }
    let api_index: RegistryIndex = serde_json::from_str(
        &fs::read_to_string(&api_index_path)
            .with_context(|| format!("reading {}", api_index_path.display()))?,
    )
    .with_context(|| format!("parsing {}", api_index_path.display()))?;
    if api_index.packages.len() != index.packages.len() {
        anyhow::bail!(
            "{} has {} packages, expected {}",
            api_index_path.display(),
            api_index.packages.len(),
            index.packages.len()
        );
    }
    Ok(index)
}

fn write_homepage(repo_dir: &Path, index: &RegistryIndex) -> Result<()> {
    fs::write(repo_dir.join("index.html"), render_homepage(index)?)?;
    Ok(())
}

fn render_homepage(index: &RegistryIndex) -> Result<String> {
    let section_counts_json = serde_json::to_string(&section_counts(&index.packages))?;
    Ok(HOMEPAGE_TEMPLATE
        .replace("__GENERATED_AT__", &html_escape(&index.generated_at))
        .replace("__PACKAGE_COUNT__", &index.packages.len().to_string())
        .replace("__SECTION_COUNTS__", &section_counts_json))
}

fn section_counts(packages: &[Package]) -> BTreeMap<String, usize> {
    let mut counts = BTreeMap::new();
    for package in packages {
        *counts.entry(package.section.clone()).or_insert(0) += 1;
    }
    counts
}

fn html_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

const HOMEPAGE_TEMPLATE: &str = r#"<!doctype html>
<html lang="en" data-smwapt-homepage>
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>smwapt</title>
  <style>
    :root {
      color-scheme: light;
      --bg: #f6f4ef;
      --panel: #ffffff;
      --ink: #171717;
      --muted: #62605a;
      --line: #d8d3c8;
      --accent: #b3212d;
      --accent-dark: #7e1420;
      --code: #242424;
      --code-bg: #efebe3;
    }
    * { box-sizing: border-box; }
    body {
      margin: 0;
      background: var(--bg);
      color: var(--ink);
      font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
      line-height: 1.5;
    }
    a { color: var(--accent-dark); text-decoration-thickness: 1px; text-underline-offset: 3px; }
    .shell { max-width: 1180px; margin: 0 auto; padding: 28px 20px 48px; }
    header {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 24px;
      align-items: end;
      padding: 28px 0 22px;
      border-bottom: 1px solid var(--line);
    }
    h1 { margin: 0; font-size: clamp(42px, 8vw, 86px); line-height: 0.95; letter-spacing: 0; }
    .lede { max-width: 720px; margin: 18px 0 0; color: var(--muted); font-size: 18px; }
    .stats {
      display: grid;
      grid-template-columns: repeat(2, minmax(130px, 1fr));
      gap: 10px;
      min-width: 300px;
    }
    .stat {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 14px;
    }
    .stat b { display: block; font-size: 28px; line-height: 1; }
    .stat span { display: block; margin-top: 6px; color: var(--muted); font-size: 13px; }
    .grid {
      display: grid;
      grid-template-columns: minmax(280px, 380px) minmax(0, 1fr);
      gap: 22px;
      align-items: start;
      margin-top: 22px;
    }
    section {
      background: var(--panel);
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 18px;
    }
    h2 { margin: 0 0 12px; font-size: 18px; }
    pre {
      margin: 0;
      overflow-x: auto;
      white-space: pre;
      color: var(--code);
      background: var(--code-bg);
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 14px;
      font: 13px/1.45 ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace;
    }
    .links { display: grid; gap: 9px; margin-top: 16px; }
    .searchbar {
      display: grid;
      grid-template-columns: minmax(0, 1fr) auto;
      gap: 10px;
      margin-bottom: 12px;
    }
    input {
      width: 100%;
      min-height: 42px;
      border: 1px solid var(--line);
      border-radius: 6px;
      padding: 9px 12px;
      font: inherit;
      color: var(--ink);
      background: #fff;
    }
    button {
      min-height: 42px;
      border: 1px solid var(--accent-dark);
      border-radius: 6px;
      padding: 0 14px;
      color: #fff;
      background: var(--accent);
      font: inherit;
      cursor: pointer;
    }
    .meta { color: var(--muted); font-size: 13px; }
    .results { display: grid; gap: 10px; margin-top: 14px; }
    .package {
      border: 1px solid var(--line);
      border-radius: 8px;
      padding: 14px;
      background: #fff;
    }
    .package h3 { margin: 0; font-size: 16px; line-height: 1.25; }
    .package .name { margin-top: 4px; color: var(--muted); font: 12px/1.4 ui-monospace, SFMono-Regular, Menlo, Consolas, "Liberation Mono", monospace; overflow-wrap: anywhere; }
    .badges { display: flex; flex-wrap: wrap; gap: 6px; margin: 10px 0 0; }
    .badge {
      border: 1px solid var(--line);
      border-radius: 999px;
      padding: 3px 8px;
      color: var(--muted);
      background: #faf9f6;
      font-size: 12px;
    }
    .package-links { display: flex; flex-wrap: wrap; gap: 10px; margin-top: 10px; font-size: 13px; }
    .sections { display: flex; flex-wrap: wrap; gap: 7px; margin-top: 12px; }
    .error { color: var(--accent-dark); }
    footer { margin-top: 24px; color: var(--muted); font-size: 13px; }
    @media (max-width: 820px) {
      header, .grid { grid-template-columns: 1fr; }
      .stats { min-width: 0; }
      .shell { padding: 18px 14px 36px; }
      .searchbar { grid-template-columns: 1fr; }
    }
  </style>
</head>
<body>
  <main class="shell">
    <header>
      <div>
        <h1>smwapt</h1>
        <p class="lede">A static package repository for Super Mario World ROM hack tools, patches, ASM, music, graphics, and other SMW Central resources.</p>
      </div>
      <div class="stats" aria-label="repository stats">
        <div class="stat"><b id="package-count">__PACKAGE_COUNT__</b><span>packages indexed</span></div>
        <div class="stat"><b id="section-count">0</b><span>sections</span></div>
      </div>
    </header>

    <div class="grid">
      <section aria-labelledby="use-source-title">
        <h2 id="use-source-title">Use This Source</h2>
        <pre>smwapt source add https://smw.sabino.pro stable main
smwapt update
smwapt search retry</pre>
        <div class="links">
          <a href="api/v1/packages.json">Package list JSON</a>
          <a href="api/v1/index.json">Full repository JSON</a>
          <a href="dists/stable/main/binary-smw/Packages.gz">Packages.gz catalog</a>
          <a href="dists/stable/Release">Release hashes</a>
        </div>
        <div class="sections" id="sections"></div>
        <p class="meta">Generated <span id="generated-at">__GENERATED_AT__</span>.</p>
      </section>

      <section aria-labelledby="search-title">
        <h2 id="search-title">Search Packages</h2>
        <form class="searchbar" id="search-form">
          <input id="query" type="search" autocomplete="off" placeholder="Search name, title, section, tag, alias, or install kind">
          <button type="submit">Search</button>
        </form>
        <div class="meta" id="result-meta">Loading package index...</div>
        <div class="results" id="results"></div>
      </section>
    </div>

    <footer>ROMs are not distributed by this repository. Package payloads are downloaded from configured upstream URLs.</footer>
  </main>

  <script>
    const initialSectionCounts = __SECTION_COUNTS__;
    const state = { packages: [] };
    const byId = (id) => document.getElementById(id);

    function fmt(value) {
      return new Intl.NumberFormat("en").format(value);
    }

    function setText(id, value) {
      byId(id).textContent = value;
    }

    function addText(parent, tag, className, value) {
      const node = document.createElement(tag);
      if (className) node.className = className;
      node.textContent = value;
      parent.appendChild(node);
      return node;
    }

    function addLink(parent, label, href) {
      const link = document.createElement("a");
      link.textContent = label;
      link.href = href;
      parent.appendChild(link);
      return link;
    }

    function renderSections(counts) {
      const holder = byId("sections");
      holder.replaceChildren();
      Object.entries(counts).forEach(([section, count]) => {
        const chip = document.createElement("span");
        chip.className = "badge";
        chip.textContent = `${section}: ${fmt(count)}`;
        holder.appendChild(chip);
      });
    }

    function latestVersion(pkg) {
      if (pkg.latest_version) return pkg.latest_version;
      if (pkg.versions && pkg.versions.length) return pkg.versions[0].version;
      return "unknown";
    }

    function packageHaystack(pkg) {
      return [
        pkg.name,
        pkg.title,
        pkg.section,
        pkg.install_kind,
        latestVersion(pkg),
        ...(pkg.aliases || []),
        ...(pkg.tags || [])
      ].join(" ").toLowerCase();
    }

    function renderPackage(pkg) {
      const row = document.createElement("article");
      row.className = "package";
      addText(row, "h3", "", pkg.title || pkg.name);
      addText(row, "div", "name", pkg.name);

      const badges = document.createElement("div");
      badges.className = "badges";
      [pkg.section, pkg.install_kind, `version ${latestVersion(pkg)}`, `smwc ${pkg.upstream_id}`]
        .filter(Boolean)
        .forEach((value) => addText(badges, "span", "badge", value));
      row.appendChild(badges);

      const links = document.createElement("div");
      links.className = "package-links";
      addLink(links, "API", `api/v1/packages/${encodeURIComponent(pkg.name)}.json`);
      const current = pkg.versions && pkg.versions.length ? pkg.versions[0] : null;
      if (current && current.download_url) addLink(links, "Download", current.download_url);
      row.appendChild(links);
      return row;
    }

    function runSearch() {
      const query = byId("query").value.trim().toLowerCase();
      const words = query.split(/\s+/).filter(Boolean);
      const matches = state.packages.filter((pkg) => {
        if (!words.length) return true;
        const haystack = packageHaystack(pkg);
        return words.every((word) => haystack.includes(word));
      });
      const shown = matches.slice(0, 100);
      const results = byId("results");
      results.replaceChildren(...shown.map(renderPackage));
      setText("result-meta", `${fmt(matches.length)} matches${matches.length > shown.length ? `, showing first ${shown.length}` : ""}`);
    }

    async function loadRepository() {
      renderSections(initialSectionCounts);
      try {
        const [indexResponse, packagesResponse] = await Promise.all([
          fetch("api/v1/index.json", { cache: "no-store" }),
          fetch("api/v1/packages.json", { cache: "no-store" })
        ]);
        if (!indexResponse.ok) throw new Error(`index HTTP ${indexResponse.status}`);
        if (!packagesResponse.ok) throw new Error(`packages HTTP ${packagesResponse.status}`);
        const index = await indexResponse.json();
        const packages = await packagesResponse.json();
        state.packages = Array.isArray(packages) ? packages : (index.packages || []);
        setText("package-count", fmt(state.packages.length));
        setText("generated-at", index.generated_at || "__GENERATED_AT__");
        const counts = {};
        state.packages.forEach((pkg) => { counts[pkg.section] = (counts[pkg.section] || 0) + 1; });
        setText("section-count", Object.keys(counts).length.toString());
        renderSections(counts);
        runSearch();
      } catch (error) {
        const meta = byId("result-meta");
        meta.replaceChildren();
        addText(meta, "span", "error", `Could not load package index: ${error.message}`);
      }
    }

    byId("search-form").addEventListener("submit", (event) => {
      event.preventDefault();
      runSearch();
    });
    byId("query").addEventListener("input", runSearch);
    setText("section-count", Object.keys(initialSectionCounts).length.toString());
    loadRepository();
  </script>
</body>
</html>
"#;

fn write_static_api(repo_dir: &Path, index: &RegistryIndex) -> Result<()> {
    let api_dir = repo_dir.join("api/v1");
    fs::create_dir_all(api_dir.join("packages"))?;
    fs::create_dir_all(api_dir.join("sections"))?;
    fs::write(
        api_dir.join("index.json"),
        serde_json::to_string_pretty(index)?,
    )?;
    fs::write(
        api_dir.join("packages.json"),
        serde_json::to_string_pretty(&index.packages)?,
    )?;

    let mut sections: BTreeMap<&str, Vec<&Package>> = BTreeMap::new();
    for package in &index.packages {
        fs::write(
            api_dir
                .join("packages")
                .join(format!("{}.json", package.name)),
            serde_json::to_string_pretty(package)?,
        )?;
        sections
            .entry(package.section.as_str())
            .or_default()
            .push(package);
    }
    for (section, packages) in sections {
        fs::write(
            api_dir.join("sections").join(format!("{section}.json")),
            serde_json::to_string_pretty(&packages)?,
        )?;
    }
    Ok(())
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
        if let Some(sha256) = &version.sha256 {
            out.push_str(&format!("SHA256: {sha256}\n"));
        }
        out.push_str(&format!("X-SMWC-ID: {}\n", pkg.upstream_id));
        out.push_str(&format!("X-SMWAPT-Kind: {:?}\n", pkg.install_kind));
        out.push_str(&format!("X-SMWAPT-URL: {}\n", version.download_url));
        out.push_str(&format!("Description: {}\n\n", one_line(&pkg.title)));
    }
    out
}

fn render_release(files: &[ReleaseFile]) -> String {
    let mut out = format!(
        "Origin: smwapt\nLabel: smwapt\nSuite: {SUITE}\nCodename: {SUITE}\nArchitectures: smw\nComponents: {COMPONENT}\nDescription: SMW package registry\n"
    );
    out.push_str("MD5Sum:\n");
    for file in files {
        out.push_str(&format!(" {} {:>16} {}\n", file.md5, file.size, file.path));
    }
    out.push_str("SHA1:\n");
    for file in files {
        out.push_str(&format!(" {} {:>16} {}\n", file.sha1, file.size, file.path));
    }
    out.push_str("SHA256:\n");
    for file in files {
        out.push_str(&format!(
            " {} {:>16} {}\n",
            file.sha256, file.size, file.path
        ));
    }
    out
}

struct ReleaseFile {
    path: String,
    size: usize,
    md5: String,
    sha1: String,
    sha256: String,
}

impl ReleaseFile {
    fn new(path: String, bytes: &[u8]) -> Self {
        Self {
            path,
            size: bytes.len(),
            md5: format!("{:x}", md5::compute(bytes)),
            sha1: hex::encode(Sha1::digest(bytes)),
            sha256: hex::encode(Sha256::digest(bytes)),
        }
    }
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
            let static_api_url = format!("{}/api/v1/packages.json", source.url);
            if let Ok(packages) = fetch_live_packages(client, &static_api_url) {
                return Ok(packages);
            }
            let static_index_url = format!("{}/api/v1/index.json", source.url);
            if let Ok(packages) = fetch_static_index(client, source, &static_index_url) {
                return Ok(packages);
            }
            let legacy_index_url = format!("{}/index.json", source.url);
            fetch_static_index(client, source, &legacy_index_url).with_context(|| {
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
        assert!(rendered.contains("Filename: pool/tools/37443/asar.zip"));
        assert!(rendered.contains("X-SMWC-ID: 37443"));
    }

    #[test]
    fn release_includes_apt_style_hashes() {
        let release = render_release(&[ReleaseFile::new(
            "main/binary-smw/Packages".to_string(),
            b"Package: asar\n",
        )]);
        assert!(release.contains("MD5Sum:\n"));
        assert!(release.contains("SHA1:\n"));
        assert!(release.contains("SHA256:\n"));
        assert!(release.contains("main/binary-smw/Packages"));
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
        assert!(dir.path().join("index.html").exists());
        assert!(!dir.path().join("index.json").exists());
        assert!(dir.path().join("api/v1/index.json").exists());
        assert!(dir.path().join("api/v1/packages.json").exists());
        assert!(dir.path().join("api/v1/packages/asar.json").exists());
        assert!(dir.path().join("api/v1/sections/tools.json").exists());
    }

    #[test]
    fn renders_static_homepage() {
        let index = RegistryIndex {
            generated_at: "2026-05-23T00:00:00Z".to_string(),
            suite: SUITE.to_string(),
            component: COMPONENT.to_string(),
            packages: vec![Package {
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
            }],
        };
        let html = render_homepage(&index).unwrap();
        assert!(html.contains("data-smwapt-homepage"));
        assert!(html.contains("api/v1/packages.json"));
        assert!(html.contains("smwapt source add https://smw.sabino.pro stable main"));
    }
}
