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
