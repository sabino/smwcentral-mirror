# Static GitHub Pages Repository

`smwapt` repositories are static by design: the live server and GitHub Pages
both expose the same package metadata model. The server adds an HTTP API, while
Pages serves JSON API files, `index.json`, and apt-style
`dists/stable/main/binary-smw` metadata files.

## Static JSON API

GitHub Pages cannot run dynamic API code, so `smwapt` generates read-only JSON
files at stable API-shaped paths:

```text
/index.json
/api/v1/index.json
/api/v1/packages.json
/api/v1/packages/<package>.json
/api/v1/sections/<section>.json
```

Examples:

```text
/api/v1/packages.json
/api/v1/packages/uberasm-retry-system.json
/api/v1/sections/uberasm.json
```

The apt-style `Packages` and `Packages.gz` files are metadata indexes for
compatibility with apt conventions. They are not package archives and do not
contain package payloads. `smwapt update` downloads the catalog; `smwapt install`
then downloads only the selected package version archive from the URL in that
catalog entry.

## Generate Locally

```sh
smwapt repo sync-smwcentral --out pages --sections tools,smwpatches,uberasm --max-pages 1
smwapt repo validate --dir pages
```

Use `--full` to mirror every page in the selected SMW Central sections:

```sh
smwapt repo sync-smwcentral --out pages --sections tools,smwpatches,uberasm,smwblocks,smwsprites,smwmusic,smwgraphics --full
```

Commit the generated `pages/` directory if you want the package index and
artifacts to be version-controlled in the same repository.

## Generate From Cached Metadata

If you already ran `smwapt update`, rebuild a static repo from the local cache:

```sh
smwapt repo build --out pages
smwapt repo validate --dir pages
```

## Use As A Source

After publishing the directory with GitHub Pages:

```sh
smwapt source add https://<owner>.github.io/<repo> stable main
smwapt update
smwapt search retry
```

`smwapt update` first tries the live server endpoint at `/api/v1/packages`.
If that is not present, it falls back to `/index.json`, which is what GitHub
Pages serves.

## Automatic Sync

The included workflow at `.github/workflows/sync-pages.yml` runs weekly and can
also be triggered manually. It generates `pages/`, validates it, and commits the
static repository to the `gh-pages` branch. It writes `smw.sabino.pro` to
`pages/CNAME`, so the generated `gh-pages` branch is ready for that custom
domain.

In GitHub repository settings, configure Pages to serve from the `gh-pages`
branch. The resulting source URL is:

```sh
smwapt source add https://<owner>.github.io/<repo> stable main
```

For this repository, the intended source is:

```sh
smwapt source add https://smw.sabino.pro stable main
```

## Integrity

The generated `Release` file includes apt-style `MD5Sum`, `SHA1`, and `SHA256`
hashes for `Packages` and `Packages.gz`. `MD5Sum` is included for apt-style
compatibility only; `SHA256` is the meaningful checksum.

Package archive metadata also supports per-version `sha256`, and `smwapt
install` rejects a downloaded archive if a package version declares a SHA-256
hash and the bytes do not match. SMW Central metadata does not currently provide
archive hashes in the synced package index, so full archive verification requires
mirroring artifacts and hashing them during publication.
