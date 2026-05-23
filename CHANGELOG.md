# Changelog

## 0.1.0

- Added Rust core, apt-style source parsing, package models, SMW Central sync, local repository generation, CLI, and HTTP server.
- Added project manifest/lockfile handling, ROM verification, backups, archive extraction, and first installer flows for tools, Asar patches, UberASM, GPS, PIXI, and AddMusicK.
- Added Qt6 GUI client and Wayland-safe launch wrapper.
- Added canonical package names with deterministic version selection through `package=version`, `--version`, and `smwapt versions`.
- Added static GitHub Pages repository generation, validation, live-server/static-source fallback, and scheduled `gh-pages` sync workflow.
- Added generated static JSON API files for package lists, package details, and per-section listings.
- Added a generated static repository homepage with source setup links and browser-side package search.
- Added SMW Graphics to the default SMW Central static mirror sections.
- Added apt-style Release hashes and package archive SHA-256 validation when hashes are present.
- Added local install script and Makefile targets for installing `smwapt` and the Wayland-safe `smwapt-gui` launcher.
- Added documentation and tests.
