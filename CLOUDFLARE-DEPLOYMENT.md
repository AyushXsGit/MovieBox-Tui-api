# Cloudflare native deployment

The complete original Rust/Axum project is intentionally preserved.

Cloudflare deployment entrypoint:
- `cloudflare/worker.js`
- `wrangler.toml`

The Cloudflare Worker is now **native** and does not use Render.

It implements the web API contract used by the Vercel frontend:
- `GET /health`
- `GET /home`
- `GET /search?q=...`
- `GET /search/suggest?q=...`
- `GET /detail/:id`
- `GET /api/stream/:subject_id?se=...&ep=...`
- `/public-proxy/...`

Required Cloudflare Worker Secret:
- `PROXY_SECRET`

The old Rust/Axum source, tests, providers, download code, and documentation remain in the repository for reference and local/other deployments. They are not the Cloudflare Worker runtime entrypoint.

Do not change the Vercel frontend API URL until the Cloudflare endpoints have been tested.
