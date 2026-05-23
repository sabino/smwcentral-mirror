# smwapt

An apt-style package manager for Super Mario World ROM hack resources.

`smwapt` manages tools, patches, and resource metadata for SMW ROM hacking
projects. It does not distribute ROMs.

## Build

```sh
cargo build
cmake -S gui -B build/gui -GNinja
cmake --build build/gui
```

Install the CLI and GUI locally:

```sh
scripts/install-local
smwapt --help
smwapt-gui
```

By default this installs into `~/.local/bin` and `~/.local/share/smwapt`.
Set `PREFIX=/some/path` to install somewhere else.

## Quick Start

Create a small local mirror from SMW Central metadata:

```sh
./target/debug/smwapt server sync --sections tools,smwpatches,uberasm,smwblocks,smwsprites --max-pages 1
./target/debug/smwapt server run
```

In another shell:

```sh
./target/debug/smwapt source add http://127.0.0.1:4789 stable main
./target/debug/smwapt update
./target/debug/smwapt search asar
```

Or generate a static repository that can be served by GitHub Pages:

```sh
smwapt repo sync-smwcentral --out pages --sections tools,smwpatches,uberasm --max-pages 1
smwapt repo validate --dir pages
```

Publish the `pages/` directory or run the included GitHub Actions workflow to
push it to a `gh-pages` branch. Then add the Pages URL as a normal apt-style
source:

```sh
smwapt source add https://<owner>.github.io/<repo> stable main
smwapt update
```

The first public repository target for this project is `https://smw.sabino.pro`.
It has a static homepage/search UI plus JSON API and apt-style metadata:

```text
/
/api/v1/index.json
/api/v1/packages.json
/api/v1/packages/<package>.json
/api/v1/sections/<section>.json
/dists/stable/main/binary-smw/Packages
/dists/stable/main/binary-smw/Packages.gz
```

`Packages.gz` is a compressed metadata index/catalog, not a package archive.
Clients download it during update, then download only the selected package's
upstream archive when installing.

Initialize a project from a verified unheadered SMW USA ROM:

```sh
./target/debug/smwapt init \
  --rom "$HOME/Downloads/Super Mario World (USA) (2).sfc" \
  --copy-rom ./hack.sfc
```

Install a tool:

```sh
./target/debug/smwapt install asar
```

Install the latest version automatically, list available versions, or pin one:

```sh
smwapt install uberasm-retry-system
smwapt versions uberasm-retry-system
smwapt install uberasm-retry-system=2.0.3 --target project
smwapt install uberasm-retry-system --version smwc-42270 --target project
```

Install resource packages with explicit targets:

```sh
./target/debug/smwapt install some-patch --entry patch.asm
./target/debug/smwapt install some-uberasm --target level:105 --entry effect.asm
./target/debug/smwapt install some-block --map16 4000 --acts-like 0130
./target/debug/smwapt install some-sprite --sprite-slot 00
./target/debug/smwapt install some-song --song-slot 29
```

## GUI

```sh
scripts/smwapt-gui-wayland
```

The GUI uses the same `smwapt` binary and project files as the CLI. On Wayland,
the wrapper and the GUI both set conservative Qt/SDL/GTK backend defaults.
After `scripts/install-local`, use `smwapt-gui`; it is installed as the same
Wayland-safe wrapper.

## Project Files

- `.smwapt/sources.list`: apt-style sources.
- `.smwapt/cache/`: cached package metadata.
- `.smwapt/repo/`: local server repository.
- `.smwapt/manifest.toml`: project ROM and resource paths.
- `.smwapt/lock.json`: installed package lockfile/history.
- `.smwapt/backups/`: ROM backups before mutation.

## Legal Boundary

`smwapt` manages metadata, tools, and patch/resource ZIPs from configured
sources. It never stores ROMs in the repository and does not distribute ROMs.

## Integrity Boundary

Repository metadata includes apt-style hashes in `Release`. Package archive
SHA-256 validation is enforced when a package version declares a `sha256` value.
Hashes prove byte integrity, not authorship; signed repository metadata should
be added before treating third-party mirrors as trusted.
