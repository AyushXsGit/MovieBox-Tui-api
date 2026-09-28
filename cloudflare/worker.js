/**
 * MovieBox TUI - Cloudflare Worker Bridge
 *
 * Keeps the existing Rust/Axum backend as the origin and puts Cloudflare in
 * front of it. The Worker does not reimplement the Rust backend.
 */
const DEFAULT_BACKEND_ORIGIN = "https://moviebox-tui-api.onrender.com";

function backendOrigin(env) {
  return String(env.BACKEND_ORIGIN || DEFAULT_BACKEND_ORIGIN).replace(/\/+$/, "");
}

function corsHeaders() {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET,HEAD,POST,OPTIONS",
    "Access-Control-Allow-Headers": "*",
    "Access-Control-Expose-Headers": "*",
    "Vary": "Origin",
  };
}

function withCors(response) {
  const headers = new Headers(response.headers);
  for (const [key, value] of Object.entries(corsHeaders())) headers.set(key, value);
  return new Response(response.body, {
    status: response.status,
    statusText: response.statusText,
    headers,
  });
}

function rewriteStreamJson(value, workerOrigin, backend) {
  if (!value || typeof value !== "object") return value;

  const rewriteUrl = (url) => {
    if (typeof url !== "string") return url;
    if (!url.startsWith(backend + "/public-proxy/")) return url;
    return workerOrigin + url.slice(backend.length);
  };

  const clone = Array.isArray(value) ? [] : {};
  for (const [key, item] of Object.entries(value)) {
    if ((key === "url" || key === "proxy_url") && typeof item === "string") {
      clone[key] = rewriteUrl(item);
    } else if (item && typeof item === "object") {
      clone[key] = rewriteStreamJson(item, workerOrigin, backend);
    } else {
      clone[key] = item;
    }
  }
  return clone;
}

async function proxyToBackend(request, env, url) {
  const backend = backendOrigin(env);
  const target = new URL(url.pathname + url.search, backend + "/");
  const headers = new Headers(request.headers);
  headers.delete("host");

  const init = {
    method: request.method,
    headers,
    redirect: "manual",
  };

  if (request.method !== "GET" && request.method !== "HEAD") {
    init.body = request.body;
  }

  return fetch(new Request(target.toString(), init));
}

export default {
  async fetch(request, env) {
    const url = new URL(request.url);

    if (request.method === "OPTIONS") {
      return new Response(null, { status: 204, headers: corsHeaders() });
    }

    const backend = backendOrigin(env);
    const response = await proxyToBackend(request, env, url);

    // The Rust backend creates /public-proxy URLs on its own origin.
    // Rewrite only those URLs in the stream JSON so the browser stays on
    // the Cloudflare hostname while the Rust proxy remains the origin.
    if (request.method === "GET" && url.pathname.startsWith("/api/stream/")) {
      const contentType = response.headers.get("content-type") || "";
      if (contentType.includes("application/json")) {
        try {
          const data = await response.json();
          const rewritten = rewriteStreamJson(data, url.origin, backend);
          const headers = new Headers(response.headers);
          headers.set("content-type", "application/json; charset=utf-8");
          for (const [key, value] of Object.entries(corsHeaders())) {
            headers.set(key, value);
          }
          return new Response(JSON.stringify(rewritten), {
            status: response.status,
            statusText: response.statusText,
            headers,
          });
        } catch (_) {
          // Return the original response if it was not valid JSON.
        }
      }
    }

    return withCors(response);
  },
};
