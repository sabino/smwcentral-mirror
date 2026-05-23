# Architecture

`smwapt` is an apt-style package manager for Super Mario World hacking projects.

The workspace is split into:

- `smwapt-core`: package models, source parsing, registry sync, ROM/project state, and installers.
- `smwapt`: command line client.
- `smwapt-server`: local/http registry server.
- `gui`: Qt6 desktop client that uses the same CLI/core behavior.
