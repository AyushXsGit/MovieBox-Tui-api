# Cloudflare bridge for MovieBox TUI API

The original Rust/Axum backend is preserved. The Cloudflare Workers bridge is
an additional layer in `cloudflare/worker.js`.

## What it does

- Proxies the existing backend routes through Cloudflare.
- Preserves `/public-proxy/...` by forwarding it to the existing Rust proxy.
- Rewrites `/api/stream/...` JSON so browser-facing `url` and `proxy_url`
  values use the Cloudflare Worker hostname.
- Preserves Range requests and streaming responses.
- Adds CORS headers.
- Does not remove the Rust backend, FFmpeg code, tests, docs, or workflows.

## Git/Cloudflare setup

Root directory: `/`
Build/deploy command: `npx wrangler deploy`

Worker name: `moviebox-tui-api-cloudflare`

The current origin is:
`https://moviebox-tui-api.onrender.com`

Keep the Render origin running until Cloudflare has been tested for `/home`,
`/search`, `/detail/...`, `/api/stream/...`, `/public-proxy/...` and the
external-player flow.

This is a Cloudflare bridge, not a conversion of the Rust/Axum server into a
native Worker. The Rust origin remains responsible for its stateful public
proxy and FFmpeg/download functionality.
