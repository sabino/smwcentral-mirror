# Architecture

`smwapt` is an apt-style package manager for Super Mario World hacking projects.

The workspace is split into:

- `smwapt-core`: package models, source parsing, registry sync, ROM/project state, and installers.
- `smwapt`: command line client.
- `smwapt-server`: local/http registry server.
- `gui`: Qt6 desktop client that uses the same CLI/core behavior.

## Data Flow

1. `smwapt server sync` reads SMW Central metadata and writes `.smwapt/repo`.
2. `smwapt server run` exposes apt-style `dists/` files and JSON APIs.
3. `smwapt source add` records the server in `.smwapt/sources.list`.
4. `smwapt update` caches package JSON under `.smwapt/cache`.
5. `smwapt install` downloads the package archive, creates a ROM backup when needed, runs the appropriate insertion tool, and records the result in `.smwapt/lock.json`.

## Installer Kinds

- `tool`: extract into `.smwapt/tools/<name>/<version>`.
- `asar_patch`: run Asar against the project ROM.
- `uberasm`: update UberASM Tool `list.txt` and run the tool.
- `gps_block`: update GPS `list.txt` and run the tool.
- `pixi_sprite`: update PIXI `list.txt` and run the tool.
- `add_music_k_music`: update `Addmusic_list.txt` and run AddMusicK.
- `asset_only`: extract into project resources without ROM mutation.
