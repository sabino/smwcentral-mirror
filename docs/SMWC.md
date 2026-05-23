# SMW Central Upstream

The first upstream is SMW Central.

The registry sync uses the structured SMW Central `ajax.php` endpoints:

- `a=getsectionlist&s=<section>&u=0`
- `a=getfile&v=2&id=<id>` when detailed records are needed later

V1 sync sections:

- `tools`
- `smwpatches`
- `uberasm`
- `smwblocks`
- `smwsprites`
- `smwmusic`

The live service is rate-limited and may be protected by Cloudflare challenges, so the server is intentionally a local mirror/cache. Use `--max-pages` during development and `--full` only when a long sync is acceptable.
