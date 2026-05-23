use crate::archive::{choose_entry, extract_zip, sha256_file};
use crate::config::Paths;
use crate::manifest::{
    read_lockfile, read_manifest, write_lockfile, write_manifest, InstallRecord, Lockfile,
    ProjectManifest,
};
use crate::package::{InstallKind, Package};
use crate::registry;
use crate::rom::verify_rom;
use anyhow::{bail, Context, Result};
use chrono::Utc;
use reqwest::blocking::Client;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Default)]
pub struct InstallOptions {
    pub entry: Option<String>,
    pub target: Option<String>,
    pub map16: Option<String>,
    pub acts_like: Option<String>,
    pub sprite_slot: Option<String>,
    pub song_slot: Option<String>,
    pub dry_run: bool,
}

pub fn init_project(root: &Path, rom: &Path, copy_rom: Option<&Path>) -> Result<ProjectManifest> {
    let selected_rom = if let Some(copy_to) = copy_rom {
        if let Some(parent) = copy_to.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::copy(rom, copy_to).with_context(|| format!("copying ROM to {}", copy_to.display()))?;
        copy_to.to_path_buf()
    } else {
        rom.to_path_buf()
    };
    verify_rom(&selected_rom)?;
    let manifest = ProjectManifest::new(&selected_rom);
    write_manifest(root, &manifest)?;
    write_lockfile(root, &Lockfile::default())?;
    fs::create_dir_all(root.join(".smwapt/backups"))?;
    fs::create_dir_all(root.join("resources"))?;
    Ok(manifest)
}

pub fn install_package(root: &Path, query: &str, options: &InstallOptions) -> Result<InstallRecord> {
    let paths = Paths::new(root);
    let packages = registry::load_cached_packages(&paths.cache_dir)?;
    let package = packages
        .iter()
        .find(|pkg| pkg.name == query || pkg.aliases.iter().any(|alias| alias == query))
        .with_context(|| format!("package {query} not found; run smwapt update first"))?
        .clone();
    install_resolved(root, &package, options, &packages)
}

fn install_resolved(
    root: &Path,
    package: &Package,
    options: &InstallOptions,
    packages: &[Package],
) -> Result<InstallRecord> {
    for dep in &package.versions[0].dependencies {
        if !is_installed(root, dep)? {
            let dep_pkg = packages
                .iter()
                .find(|pkg| pkg.name == *dep || pkg.aliases.iter().any(|alias| alias == dep))
                .with_context(|| format!("dependency {dep} not available"))?
                .clone();
            let dep_options = InstallOptions {
                dry_run: options.dry_run,
                ..Default::default()
            };
            install_resolved(root, &dep_pkg, &dep_options, packages)?;
        }
    }

    let manifest = read_manifest(root).context("project is not initialized; run smwapt init")?;
    let version = &package.versions[0];
    let archive = download_archive(root, package)?;
    let sha256 = sha256_file(&archive)?;
    let install_dir = install_dir(root, package);
    if !options.dry_run {
        if install_dir.exists() {
            fs::remove_dir_all(&install_dir)?;
        }
        extract_zip(&archive, &install_dir)?;
    }
    let entries = crate::archive::list_zip(&archive)?;
    let selected_entry = select_entry(package.install_kind, &entries, options)?;
    let backup = if package.install_kind == InstallKind::Tool || options.dry_run {
        None
    } else {
        Some(backup_rom(root, Path::new(&manifest.rom))?)
    };
    let command = if !options.dry_run {
        run_installer(root, package, &install_dir, &manifest, selected_entry.as_deref(), options)?
    } else {
        None
    };
    let record = InstallRecord {
        name: package.name.clone(),
        version: version.version.clone(),
        upstream_id: package.upstream_id,
        installed_at: Utc::now().to_rfc3339(),
        install_kind: package.install_kind,
        archive_sha256: Some(sha256),
        selected_entry,
        target: effective_target(package.install_kind, options),
        backup,
        command,
        status: if options.dry_run { "dry-run" } else { "installed" }.to_string(),
    };
    if !options.dry_run {
        let mut lock = read_lockfile(root)?;
        lock.installed.retain(|existing| existing.name != record.name);
        lock.installed.push(record.clone());
        write_lockfile(root, &lock)?;
    }
    Ok(record)
}

fn download_archive(root: &Path, package: &Package) -> Result<PathBuf> {
    let paths = Paths::new(root);
    fs::create_dir_all(&paths.downloads_dir)?;
    let version = &package.versions[0];
    let path = paths
        .downloads_dir
        .join(format!("{}-{}-{}", package.name, package.upstream_id, version.filename));
    if path.exists() {
        return Ok(path);
    }
    let client = Client::builder()
        .user_agent("smwapt/0.1")
        .timeout(std::time::Duration::from_secs(60))
        .build()?;
    let mut response = client
        .get(&version.download_url)
        .send()
        .with_context(|| format!("downloading {}", version.download_url))?
        .error_for_status()
        .with_context(|| format!("downloading {}", version.download_url))?;
    let mut file = fs::File::create(&path)?;
    response.copy_to(&mut file)?;
    Ok(path)
}

fn install_dir(root: &Path, package: &Package) -> PathBuf {
    match package.install_kind {
        InstallKind::Tool => root
            .join(".smwapt/tools")
            .join(&package.name)
            .join(&package.versions[0].version),
        _ => root.join("resources").join(&package.name),
    }
}

fn select_entry(
    kind: InstallKind,
    entries: &[crate::archive::ArchiveEntry],
    options: &InstallOptions,
) -> Result<Option<String>> {
    match kind {
        InstallKind::Tool | InstallKind::AssetOnly => Ok(None),
        InstallKind::AsarPatch | InstallKind::UberAsm | InstallKind::GpsBlock => {
            Ok(Some(choose_entry(entries, options.entry.as_deref(), ".asm")?))
        }
        InstallKind::PixiSprite => {
            if let Ok(entry) = choose_entry(entries, options.entry.as_deref(), ".json") {
                Ok(Some(entry))
            } else {
                Ok(Some(choose_entry(entries, options.entry.as_deref(), ".cfg")?))
            }
        }
        InstallKind::AddMusicKMusic => Ok(Some(choose_entry(entries, options.entry.as_deref(), ".txt")?)),
    }
}

fn backup_rom(root: &Path, rom: &Path) -> Result<String> {
    verify_rom(rom)?;
    let dir = root.join(".smwapt/backups");
    fs::create_dir_all(&dir)?;
    let dest = dir.join(format!("{}.sfc", Utc::now().format("%Y%m%d%H%M%S")));
    fs::copy(rom, &dest)?;
    Ok(dest.display().to_string())
}

fn run_installer(
    root: &Path,
    package: &Package,
    install_dir: &Path,
    manifest: &ProjectManifest,
    selected_entry: Option<&str>,
    options: &InstallOptions,
) -> Result<Option<String>> {
    match package.install_kind {
        InstallKind::Tool | InstallKind::AssetOnly => Ok(None),
        InstallKind::AsarPatch => run_asar(root, install_dir, selected_entry.context("missing asm entry")?, &manifest.rom),
        InstallKind::UberAsm => run_uberasm(root, install_dir, selected_entry.context("missing asm entry")?, &manifest.rom, options),
        InstallKind::GpsBlock => run_gps(root, install_dir, selected_entry.context("missing asm entry")?, &manifest.rom, options),
        InstallKind::PixiSprite => run_pixi(root, install_dir, selected_entry.context("missing sprite entry")?, &manifest.rom, options),
        InstallKind::AddMusicKMusic => run_addmusick(root, install_dir, selected_entry.context("missing music entry")?, &manifest.rom, options),
    }
}

fn run_asar(root: &Path, install_dir: &Path, entry: &str, rom: &str) -> Result<Option<String>> {
    let asar = find_tool_executable(root, "asar", "asar.exe")?;
    let patch = install_dir.join(entry);
    run_command(asar.parent().unwrap_or(root), &asar, &[patch.as_path(), Path::new(rom)])
}

fn run_uberasm(root: &Path, install_dir: &Path, entry: &str, rom: &str, options: &InstallOptions) -> Result<Option<String>> {
    let target = options.target.as_deref().context("UberASM install requires --target")?;
    let tool_dir = tool_dir(root, "uberasm-tool")?;
    let bucket = target.split(':').next().unwrap_or("level");
    let dest_subdir = match bucket {
        "global" | "statusbar" => "other",
        "library" => "library",
        "gamemode" => "gamemode",
        "overworld" => "overworld",
        _ => "level",
    };
    let dest_name = Path::new(entry).file_name().context("entry filename")?;
    fs::create_dir_all(tool_dir.join(dest_subdir))?;
    fs::copy(install_dir.join(entry), tool_dir.join(dest_subdir).join(dest_name))?;
    write_uberasm_list(&tool_dir, target, dest_subdir, dest_name.to_string_lossy().as_ref(), rom)?;
    let exe = find_in_dir(&tool_dir, "UberASMTool.exe")?;
    run_command(&tool_dir, &exe, &[Path::new("list.txt"), Path::new(rom)])
}

fn run_gps(root: &Path, install_dir: &Path, entry: &str, rom: &str, options: &InstallOptions) -> Result<Option<String>> {
    let map16 = options.map16.as_deref().context("GPS block install requires --map16")?;
    let acts_like = options.acts_like.as_deref().unwrap_or("0130");
    let tool_dir = tool_dir(root, "gps")?;
    let dest_name = Path::new(entry).file_name().context("entry filename")?;
    fs::create_dir_all(tool_dir.join("blocks"))?;
    fs::copy(install_dir.join(entry), tool_dir.join("blocks").join(dest_name))?;
    fs::write(tool_dir.join("list.txt"), format!("{map16}:{acts_like} {}\n", dest_name.to_string_lossy()))?;
    let exe = find_in_dir(&tool_dir, "gps.exe")?;
    run_command(&tool_dir, &exe, &[Path::new(rom)])
}

fn run_pixi(root: &Path, install_dir: &Path, entry: &str, rom: &str, options: &InstallOptions) -> Result<Option<String>> {
    let slot = options.sprite_slot.as_deref().context("PIXI sprite install requires --sprite-slot")?;
    let tool_dir = tool_dir(root, "pixi")?;
    let dest_name = Path::new(entry).file_name().context("entry filename")?;
    fs::create_dir_all(tool_dir.join("sprites"))?;
    fs::copy(install_dir.join(entry), tool_dir.join("sprites").join(dest_name))?;
    fs::write(tool_dir.join("list.txt"), format!("{slot} {}\n", dest_name.to_string_lossy()))?;
    let exe = find_in_dir(&tool_dir, "pixi.exe")?;
    run_command(&tool_dir, &exe, &[Path::new(rom)])
}

fn run_addmusick(root: &Path, install_dir: &Path, entry: &str, rom: &str, options: &InstallOptions) -> Result<Option<String>> {
    let slot = options.song_slot.as_deref().context("AddMusicK install requires --song-slot")?;
    let tool_dir = tool_dir(root, "addmusick")?;
    let dest_name = Path::new(entry).file_name().context("entry filename")?;
    fs::create_dir_all(tool_dir.join("music/smwapt"))?;
    fs::copy(install_dir.join(entry), tool_dir.join("music/smwapt").join(dest_name))?;
    fs::write(tool_dir.join("Addmusic_list.txt"), format!("{slot} music/smwapt/{}\n", dest_name.to_string_lossy()))?;
    let exe = find_in_dir(&tool_dir, "AddmusicK.exe")?;
    run_command(&tool_dir, &exe, &[Path::new(rom)])
}

fn write_uberasm_list(tool_dir: &Path, target: &str, subdir: &str, filename: &str, rom: &str) -> Result<()> {
    let mut content = format!("rom: {rom}\n\n");
    match target.split_once(':') {
        Some(("level", value)) => content.push_str(&format!("level:\n{value} {subdir}/{filename}\n")),
        Some(("gamemode", value)) => content.push_str(&format!("gamemode:\n{value} {subdir}/{filename}\n")),
        Some(("overworld", value)) => content.push_str(&format!("overworld:\n{value} {subdir}/{filename}\n")),
        Some(("library", _)) | Some(("global", _)) | Some(("statusbar", _)) | None => {
            content.push_str(&format!("global:\n{subdir}/{filename}\n"))
        }
        Some((other, _)) => bail!("unsupported UberASM target kind {other}"),
    }
    fs::write(tool_dir.join("list.txt"), content)?;
    Ok(())
}

fn run_command(cwd: &Path, exe: &Path, args: &[&Path]) -> Result<Option<String>> {
    let mut command = if exe.extension().and_then(|ext| ext.to_str()).map(|ext| ext.eq_ignore_ascii_case("exe")).unwrap_or(false) && !cfg!(windows) {
        let mut cmd = Command::new("wine");
        cmd.arg(exe);
        cmd
    } else {
        Command::new(exe)
    };
    apply_visible_env(&mut command);
    command.current_dir(cwd);
    for arg in args {
        command.arg(arg);
    }
    let printable = format!("{:?}", command);
    let output = command.output().with_context(|| format!("running {printable}"))?;
    if !output.status.success() {
        bail!(
            "installer failed: {}\nstdout:\n{}\nstderr:\n{}",
            printable,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(Some(printable))
}

pub fn apply_visible_env(command: &mut Command) {
    if env::var_os("WAYLAND_DISPLAY").is_some() {
        command.env("QT_QPA_PLATFORM", env::var("QT_QPA_PLATFORM").unwrap_or_else(|_| "wayland;xcb".to_string()));
        command.env("SDL_VIDEODRIVER", env::var("SDL_VIDEODRIVER").unwrap_or_else(|_| "wayland".to_string()));
        command.env("GDK_BACKEND", env::var("GDK_BACKEND").unwrap_or_else(|_| "wayland,x11".to_string()));
    }
    command.env("NO_AT_BRIDGE", env::var("NO_AT_BRIDGE").unwrap_or_else(|_| "1".to_string()));
}

fn is_installed(root: &Path, name: &str) -> Result<bool> {
    Ok(read_lockfile(root)?.installed.iter().any(|record| record.name == name))
}

fn tool_dir(root: &Path, name: &str) -> Result<PathBuf> {
    let base = root.join(".smwapt/tools").join(name);
    let mut versions = fs::read_dir(&base)
        .with_context(|| format!("tool {name} is not installed; run smwapt install {name}"))?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .collect::<Vec<_>>();
    versions.sort();
    versions.pop().with_context(|| format!("tool {name} is not installed"))
}

fn find_tool_executable(root: &Path, tool: &str, exe: &str) -> Result<PathBuf> {
    let dir = tool_dir(root, tool)?;
    find_in_dir(&dir, exe)
}

fn find_in_dir(dir: &Path, exe: &str) -> Result<PathBuf> {
    for entry in walkdir::WalkDir::new(dir) {
        let entry = entry?;
        if entry.file_type().is_file() && entry.file_name().to_string_lossy().eq_ignore_ascii_case(exe) {
            return Ok(entry.path().to_path_buf());
        }
    }
    bail!("{} not found under {}", exe, dir.display())
}

fn effective_target(kind: InstallKind, options: &InstallOptions) -> Option<String> {
    match kind {
        InstallKind::UberAsm => options.target.clone(),
        InstallKind::GpsBlock => options.map16.clone(),
        InstallKind::PixiSprite => options.sprite_slot.clone(),
        InstallKind::AddMusicKMusic => options.song_slot.clone(),
        _ => None,
    }
}
