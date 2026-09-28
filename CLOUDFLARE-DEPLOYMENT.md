# Cloudflare deployment note

This repository contains the original Rust/Axum backend plus an additional
Cloudflare Workers bridge.

- Worker entrypoint: `cloudflare/worker.js`
- Worker config: `wrangler.toml`
- Backend origin: configured by `BACKEND_ORIGIN`

Original backend files are intentionally preserved.
