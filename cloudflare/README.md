# Cloudflare Worker

This directory contains the native Cloudflare Worker used for the web API.

It replaces the previous Worker bridge that forwarded requests to Render.

## Required secret

`PROXY_SECRET`

Create it in Cloudflare Workers -> Settings -> Variables and Secrets -> Secrets.

## API

- `/health`
- `/home`
- `/search?q=...`
- `/search/suggest?q=...`
- `/detail/:id`
- `/api/stream/:id?se=...&ep=...`
- `/public-proxy/...`

The original Rust implementation remains under `src/` and is not removed.
