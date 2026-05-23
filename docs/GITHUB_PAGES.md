# Static GitHub Pages Repository

`smwapt` repositories are static by design: the live server and GitHub Pages
both expose the same package metadata model. The server adds an HTTP API, while
Pages serves `index.json` plus apt-style `dists/stable/main/binary-smw` files.

## Generate Locally

```sh
smwapt repo sync-smwcentral --out pages --sections tools,smwpatches,uberasm --max-pages 1
smwapt repo validate --dir pages
```

Use `--full` to mirror every page in the selected SMW Central sections:

```sh
smwapt repo sync-smwcentral --out pages --sections tools,smwpatches,uberasm,smwblocks,smwsprites,smwmusic --full
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
static repository to the `gh-pages` branch.

In GitHub repository settings, configure Pages to serve from the `gh-pages`
branch. The resulting source URL is:

```sh
smwapt source add https://<owner>.github.io/<repo> stable main
```
