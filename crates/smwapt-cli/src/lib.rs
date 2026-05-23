use anyhow::{Context, Result};
use clap::{Args, Parser, Subcommand};
use smwapt_core::config::Paths;
use smwapt_core::install::{init_project, install_package, InstallOptions};
use smwapt_core::registry::{
    cache_packages_from_sources, load_cached_packages, validate_repository, write_repository,
};
use smwapt_core::rom::inspect_rom;
use smwapt_core::smwc::{sync_packages, SyncOptions};
use smwapt_core::sources::{read_sources, write_sources, Source};
use smwapt_server::server::{run_server, ServerOptions};
use std::net::SocketAddr;
use std::path::{Path, PathBuf};

#[derive(Debug, Parser)]
#[command(
    name = "smwapt",
    version,
    about = "Apt-style package manager for SMW hacking resources"
)]
struct Cli {
    #[arg(long, default_value = ".")]
    root: PathBuf,
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Source {
        #[command(subcommand)]
        command: SourceCommand,
    },
    Update,
    Search {
        query: String,
    },
    Show {
        package: String,
    },
    Versions {
        package: String,
    },
    Policy {
        package: String,
    },
    List,
    Init {
        #[arg(long)]
        rom: PathBuf,
        #[arg(long)]
        copy_rom: Option<PathBuf>,
    },
    Install {
        package: String,
        #[command(flatten)]
        options: InstallCliOptions,
    },
    Remove {
        package: String,
    },
    Upgrade,
    History,
    Rollback {
        id: String,
    },
    Rom {
        #[command(subcommand)]
        command: RomCommand,
    },
    Doctor,
    Server {
        #[command(subcommand)]
        command: ServerCommand,
    },
    Repo {
        #[command(subcommand)]
        command: RepoCommand,
    },
}

#[derive(Debug, Subcommand)]
enum SourceCommand {
    List,
    Add {
        url: String,
        #[arg(default_value = "stable")]
        suite: String,
        #[arg(default_value = "main")]
        component: String,
    },
    Remove {
        url: String,
    },
}

#[derive(Debug, Subcommand)]
enum RomCommand {
    Verify {
        #[arg(long)]
        rom: Option<PathBuf>,
    },
}

#[derive(Debug, Subcommand)]
enum ServerCommand {
    Run {
        #[arg(long, default_value = "127.0.0.1:4789")]
        bind: SocketAddr,
        #[arg(long)]
        repo_dir: Option<PathBuf>,
    },
    Sync {
        #[arg(long)]
        repo_dir: Option<PathBuf>,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        max_pages: Option<u64>,
        #[arg(long, value_delimiter = ',')]
        sections: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum RepoCommand {
    SyncSmwcentral {
        #[arg(long, default_value = "pages")]
        out: PathBuf,
        #[arg(long)]
        full: bool,
        #[arg(long)]
        max_pages: Option<u64>,
        #[arg(long, value_delimiter = ',')]
        sections: Vec<String>,
    },
    Build {
        #[arg(long, default_value = "pages")]
        out: PathBuf,
    },
    Validate {
        #[arg(long, default_value = "pages")]
        dir: PathBuf,
    },
}

#[derive(Debug, Args, Default)]
struct InstallCliOptions {
    #[arg(long)]
    entry: Option<String>,
    #[arg(long)]
    version: Option<String>,
    #[arg(long)]
    target: Option<String>,
    #[arg(long)]
    map16: Option<String>,
    #[arg(long)]
    acts_like: Option<String>,
    #[arg(long)]
    sprite_slot: Option<String>,
    #[arg(long)]
    song_slot: Option<String>,
    #[arg(long)]
    dry_run: bool,
}

impl From<InstallCliOptions> for InstallOptions {
    fn from(value: InstallCliOptions) -> Self {
        Self {
            entry: value.entry,
            version: value.version,
            target: value.target,
            map16: value.map16,
            acts_like: value.acts_like,
            sprite_slot: value.sprite_slot,
            song_slot: value.song_slot,
            dry_run: value.dry_run,
        }
    }
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let root = cli.root.canonicalize().unwrap_or(cli.root);
    let paths = Paths::new(&root);
    match cli.command {
        Command::Source { command } => handle_source(&paths, command),
        Command::Update => {
            let sources = read_sources(&paths.sources_list)?;
            if sources.is_empty() {
                println!("no sources configured; try: smwapt source add http://127.0.0.1:4789 stable main");
                return Ok(());
            }
            let packages = cache_packages_from_sources(&sources, &paths.cache_dir)?;
            println!("Fetched {} packages.", packages.len());
            Ok(())
        }
        Command::Search { query } => {
            let packages = load_cached_packages(&paths.cache_dir)?;
            for pkg in packages.iter().filter(|pkg| {
                let q = query.to_ascii_lowercase();
                pkg.name.contains(&q)
                    || pkg.title.to_ascii_lowercase().contains(&q)
                    || pkg.aliases.iter().any(|alias| alias.contains(&q))
            }) {
                println!("{:<32} {} [{}]", pkg.name, pkg.title, pkg.section);
            }
            Ok(())
        }
        Command::Show { package } | Command::Policy { package } => {
            let packages = load_cached_packages(&paths.cache_dir)?;
            let pkg = packages
                .iter()
                .find(|pkg| {
                    pkg.name == package || pkg.aliases.iter().any(|alias| alias == &package)
                })
                .with_context(|| format!("package {package} not found"))?;
            println!("{}", serde_json::to_string_pretty(pkg)?);
            Ok(())
        }
        Command::Versions { package } => {
            let packages = load_cached_packages(&paths.cache_dir)?;
            let pkg = packages
                .iter()
                .find(|pkg| {
                    pkg.name == package || pkg.aliases.iter().any(|alias| alias == &package)
                })
                .with_context(|| format!("package {package} not found"))?;
            for version in &pkg.versions {
                println!(
                    "{}\tsmwc-{}\t{}\t{}",
                    version.version, version.upstream_id, version.title, version.download_url
                );
            }
            Ok(())
        }
        Command::List => {
            let lock = smwapt_core::manifest::read_lockfile(&root)?;
            for record in lock.installed {
                println!(
                    "{}\t{}\t{}\t{}",
                    record.name, record.version, record.status, record.installed_at
                );
            }
            Ok(())
        }
        Command::Init { rom, copy_rom } => {
            let manifest = init_project(&root, &rom, copy_rom.as_deref())?;
            println!("initialized project with ROM {}", manifest.rom);
            Ok(())
        }
        Command::Install { package, options } => {
            let record = install_package(&root, &package, &options.into())?;
            println!("{}", serde_json::to_string_pretty(&record)?);
            Ok(())
        }
        Command::Remove { package } => {
            let mut lock = smwapt_core::manifest::read_lockfile(&root)?;
            let before = lock.installed.len();
            lock.installed.retain(|record| record.name != package);
            smwapt_core::manifest::write_lockfile(&root, &lock)?;
            println!("removed {} lock entries", before - lock.installed.len());
            Ok(())
        }
        Command::Upgrade => {
            println!("upgrade is available as reinstall in 0.1.0: run smwapt update, then smwapt install <package>");
            Ok(())
        }
        Command::History => {
            let lock = smwapt_core::manifest::read_lockfile(&root)?;
            println!("{}", serde_json::to_string_pretty(&lock.installed)?);
            Ok(())
        }
        Command::Rollback { id } => {
            let lock = smwapt_core::manifest::read_lockfile(&root)?;
            let manifest = smwapt_core::manifest::read_manifest(&root)?;
            let record = lock
                .installed
                .iter()
                .find(|record| record.name == id || record.installed_at.starts_with(&id))
                .with_context(|| format!("history id/package {id} not found"))?;
            let backup = record
                .backup
                .as_ref()
                .context("selected record has no ROM backup")?;
            std::fs::copy(backup, &manifest.rom)?;
            println!("restored {}", backup);
            Ok(())
        }
        Command::Rom { command } => match command {
            RomCommand::Verify { rom } => {
                let rom = rom.unwrap_or_else(|| {
                    smwapt_core::manifest::read_manifest(&root)
                        .map(|m| PathBuf::from(m.rom))
                        .unwrap_or_else(|_| {
                            PathBuf::from("/home/sabino/Downloads/Super Mario World (USA) (2).sfc")
                        })
                });
                let info = inspect_rom(&rom)?;
                println!(
                    "{}\nsize={}\nsha1={}\nvalid_unheadered_usa={}",
                    info.path, info.size, info.sha1, info.valid_unheadered_usa
                );
                Ok(())
            }
        },
        Command::Doctor => handle_doctor(&root),
        Command::Server { command } => handle_server(&paths, command),
        Command::Repo { command } => handle_repo(&paths, command),
    }
}

fn handle_source(paths: &Paths, command: SourceCommand) -> Result<()> {
    let mut sources = read_sources(&paths.sources_list)?;
    match command {
        SourceCommand::List => {
            for source in sources {
                println!("{source}");
            }
        }
        SourceCommand::Add {
            url,
            suite,
            component,
        } => {
            let source = Source::parse(&format!("deb {url} {suite} {component}"))?;
            if !sources.contains(&source) {
                sources.push(source);
            }
            write_sources(&paths.sources_list, &sources)?;
            println!("sources written to {}", paths.sources_list.display());
        }
        SourceCommand::Remove { url } => {
            sources.retain(|source| source.url != url.trim_end_matches('/'));
            write_sources(&paths.sources_list, &sources)?;
            println!("sources written to {}", paths.sources_list.display());
        }
    }
    Ok(())
}

fn handle_server(paths: &Paths, command: ServerCommand) -> Result<()> {
    match command {
        ServerCommand::Run { bind, repo_dir } => {
            let repo_dir = repo_dir.unwrap_or_else(|| paths.repo_dir.clone());
            let rt = tokio::runtime::Runtime::new()?;
            println!("serving {} at http://{}", repo_dir.display(), bind);
            rt.block_on(run_server(ServerOptions { repo_dir, bind }))
        }
        ServerCommand::Sync {
            repo_dir,
            full,
            max_pages,
            sections,
        } => {
            let repo_dir = repo_dir.unwrap_or_else(|| paths.repo_dir.clone());
            let options = sync_options(full, max_pages, sections);
            let packages = sync_packages(&options)?;
            write_repository(&repo_dir, packages.clone())?;
            println!(
                "synced {} packages into {}",
                packages.len(),
                repo_dir.display()
            );
            Ok(())
        }
    }
}

fn handle_repo(paths: &Paths, command: RepoCommand) -> Result<()> {
    match command {
        RepoCommand::SyncSmwcentral {
            out,
            full,
            max_pages,
            sections,
        } => {
            let options = sync_options(full, max_pages, sections);
            let packages = sync_packages(&options)?;
            let index = write_repository(&out, packages)?;
            println!(
                "wrote {} packages into static repository {}",
                index.packages.len(),
                out.display()
            );
            println!("publish that directory with GitHub Pages, then add it as a source:");
            println!("smwapt source add https://<owner>.github.io/<repo> stable main");
            Ok(())
        }
        RepoCommand::Build { out } => {
            let packages = load_cached_packages(&paths.cache_dir)?;
            if packages.is_empty() {
                anyhow::bail!("no cached packages found; run smwapt update or smwapt repo sync-smwcentral first");
            }
            let index = write_repository(&out, packages)?;
            println!(
                "wrote {} cached packages into static repository {}",
                index.packages.len(),
                out.display()
            );
            Ok(())
        }
        RepoCommand::Validate { dir } => {
            let index = validate_repository(&dir)?;
            println!(
                "valid static repository: {} packages in {}",
                index.packages.len(),
                dir.display()
            );
            Ok(())
        }
    }
}

fn sync_options(full: bool, max_pages: Option<u64>, sections: Vec<String>) -> SyncOptions {
    let mut options = SyncOptions::default();
    if full {
        options.max_pages = None;
    } else if max_pages.is_some() {
        options.max_pages = max_pages;
    }
    if !sections.is_empty() {
        options.sections = sections;
    }
    options
}

fn handle_doctor(root: &Path) -> Result<()> {
    println!("root={}", root.display());
    for tool in ["wine", "unzip", "7z"] {
        println!("{}={}", tool, command_exists(tool));
    }
    let paths = Paths::new(root);
    println!("sources={}", paths.sources_list.display());
    println!(
        "cached_packages={}",
        load_cached_packages(&paths.cache_dir)?.len()
    );
    if let Ok(manifest) = smwapt_core::manifest::read_manifest(root) {
        let info = inspect_rom(Path::new(&manifest.rom))?;
        println!(
            "rom={} valid_unheadered_usa={}",
            info.path, info.valid_unheadered_usa
        );
    } else {
        println!("project=not initialized");
    }
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        println!("wayland=detected; visible launch defaults use QT_QPA_PLATFORM=wayland;xcb and SDL_VIDEODRIVER=wayland");
    }
    Ok(())
}

fn command_exists(name: &str) -> bool {
    std::env::var_os("PATH")
        .and_then(|paths| {
            std::env::split_paths(&paths)
                .map(|path| path.join(name))
                .find(|path| path.exists())
        })
        .is_some()
}
