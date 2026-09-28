/**
 * MovieBox TUI - Cloudflare-native API
 *
 * This Worker replaces the old Cloudflare -> Render bridge.
 * The original Rust/Axum source tree is intentionally preserved in the repo.
 *
 * Required secret:
 *   PROXY_SECRET
 *
 * The Worker talks directly to the MovieBox upstream API, reproduces the
 * request signing used by src/providers/moviebox/crypto.rs, adapts responses
 * to the existing frontend API contract, and provides a signed public stream
 * proxy for DASH manifests/segments.
 */

import { Buffer } from "node:buffer";
import { createHash, createHmac, randomBytes, randomUUID } from "node:crypto";

const HOST_POOL = [
  "https://api6.aoneroom.com",
  "https://api5.aoneroom.com",
  "https://api4.aoneroom.com",
  "https://api4sg.aoneroom.com",
  "https://api3.aoneroom.com",
  "https://api6sg.aoneroom.com",
  "https://api.inmoviebox.com",
];

const RETRY_STATUS_CODES = new Set([403, 406, 407, 429, 500, 502, 503, 504]);
const STREAM_REFERER = "https://sportslive.wine";
const DEFAULT_PROXY_TTL = 2 * 60 * 60;

let sessionToken = null;
let sessionExpiresAt = 0;
let activeBaseIndex = 0;
let clientProfile = null;

function requireProxySecret(env) {
  const secret = String(env?.PROXY_SECRET || "").trim();
  if (!secret) throw new Error("PROXY_SECRET is not configured");
  return secret;
}

function corsHeaders() {
  return {
    "Access-Control-Allow-Origin": "*",
    "Access-Control-Allow-Methods": "GET,HEAD,OPTIONS",
    "Access-Control-Allow-Headers": "*",
    "Access-Control-Expose-Headers": "*",
  };
}

function jsonResponse(value, status = 200, extra = {}) {
  const headers = new Headers({
    "Content-Type": "application/json; charset=utf-8",
    ...corsHeaders(),
    ...extra,
  });
  return new Response(JSON.stringify(value), { status, headers });
}

function errorResponse(message, status = 502) {
  return jsonResponse({ error: String(message) }, status);
}

function randomHex(bytes = 16) {
  return randomBytes(bytes).toString("hex");
}

function randomIp() {
  const prefixes = [
    "103.241", "49.36", "117.195", "106.198", "122.162",
    "157.32", "182.70", "103.58", "27.60", "59.90",
  ];
  const prefix = prefixes[randomBytes(1)[0] % prefixes.length];
  const a = 1 + (randomBytes(1)[0] % 253);
  const b = 1 + (randomBytes(1)[0] % 253);
  return `${prefix}.${a}.${b}`;
}

function makeClientProfile() {
  const androidVersions = [
    ["9", "PQ3A.190605.03081104"],
    ["10", "QP1A.191005.007.A3"],
    ["11", "RP1A.200720.011"],
    ["12", "S1B.220414.015"],
    ["13", "TQ2A.230405.003"],
  ];
  const devices = [
    ["23078RKD5C", "Redmi"],
    ["2201117TY", "Redmi"],
    ["2201117TG", "Redmi"],
    ["22101316G", "Redmi"],
    ["21121210G", "Redmi"],
    ["M2012K11AG", "Redmi"],
    ["M2007J20CG", "Redmi"],
  ];
  const versionCodes = [50020117, 50020118, 50020119, 50020120, 50020121];
  const networks = ["NETWORK_WIFI", "NETWORK_MOBILE"];
  const timezones = [
    "Asia/Kolkata", "Asia/Shanghai", "Asia/Tokyo",
    "America/New_York", "Europe/London",
  ];

  const android = androidVersions[randomBytes(1)[0] % androidVersions.length];
  const device = devices[randomBytes(1)[0] % devices.length];
  const versionCode = versionCodes[randomBytes(1)[0] % versionCodes.length];
  const network = networks[randomBytes(1)[0] % networks.length];
  const timezone = timezones[randomBytes(1)[0] % timezones.length];

  const gaid = randomUUID();
  const deviceId = randomHex(16);

  const userAgent =
    `com.community.oneroom/${versionCode} (Linux; U; Android ${android[0]}; ` +
    `en_US; ${device[0]}; Build/${android[1]}; Cronet/135.0.7012.3)`;

  const clientInfo = JSON.stringify({
    package_name: "com.community.oneroom",
    version_name: "4.0.01.0813.03",
    version_code: versionCode,
    os: "android",
    os_version: android[0],
    install_ch: "ps",
    device_id: deviceId,
    install_store: "ps",
    gaid,
    brand: device[1],
    model: device[0],
    system_language: "en",
    net: network,
    region: "US",
    timezone,
    sp_code: "40401",
    "X-Play-Mode": "2",
  });

  return { userAgent, clientInfo, spoofedIp: randomIp() };
}

function md5Hex(data) {
  return createHash("md5").update(data).digest("hex");
}

function b64(data) {
  return Buffer.from(data).toString("base64");
}

function sortedQueryString(url) {
  const parsed = new URL(url);
  const entries = [...parsed.searchParams.entries()].sort((a, b) => {
    const key = a[0].localeCompare(b[0]);
    return key || a[1].localeCompare(b[1]);
  });
  return entries.map(([k, v]) => `${k}=${v}`).join("&");
}

const SIGNATURE_SECRET = Buffer.from(
  "efa891974eecd3148df63aa611602defd101259ba521022c57ae0566bd8e",
  "hex"
);

function signedHeaders(method, url, body, authToken) {
  // Generate per-request client identity inside the request path. Cloudflare
  // Workers disallow crypto random operations during module initialization.
  const profile = clientProfile || (clientProfile = makeClientProfile());
  const timestamp = Date.now();
  const bodyText = body ?? "";
  const bodyBytes = Buffer.from(bodyText);
  const truncated = bodyBytes.subarray(0, 102400);
  const bodyHash = bodyText ? md5Hex(truncated) : "";
  const bodyLength = bodyText ? String(bodyBytes.length) : "";
  const parsed = new URL(url);
  const query = sortedQueryString(url);
  const canonicalUrl = query ? `${parsed.pathname}?${query}` : parsed.pathname;

  const canonical = [
    method.toUpperCase(),
    "application/json",
    "application/json",
    bodyLength,
    timestamp,
    bodyHash,
    canonicalUrl,
  ].join("\n");

  const hmac = createHmac("md5", SIGNATURE_SECRET)
    .update(canonical)
    .digest("base64");

  const reversedTimestamp = String(timestamp).split("").reverse().join("");
  const clientToken = `${timestamp},${md5Hex(reversedTimestamp)}`;
  const signature = `${timestamp}|2|${hmac}`;

  const headers = {
    "User-Agent": profile.userAgent,
    "Accept": "application/json",
    "Content-Type": "application/json",
    "Connection": "keep-alive",
    "x-client-token": clientToken,
    "x-tr-signature": signature,
    "x-client-info": profile.clientInfo,
    "x-client-status": "0",
    "x-forwarded-for": profile.spoofedIp,
  };

  if (authToken) headers.Authorization = `Bearer ${authToken}`;
  return headers;
}

function extractToken(payload) {
  const token = payload?.token;
  return typeof token === "string" && token.trim() ? token : null;
}

function tokenExpiry(token) {
  try {
    const part = String(token).split(".")[1];
    if (!part) return 0;
    const normalized = part.replace(/-/g, "+").replace(/_/g, "/");
    const json = JSON.parse(Buffer.from(normalized, "base64").toString("utf8"));
    return Number(json.exp || 0) * 1000;
  } catch {
    return 0;
  }
}

async function upstreamRequest(path, method = "GET", body = null) {
  let token = sessionToken;
  if (!token || (sessionExpiresAt && Date.now() >= sessionExpiresAt - 30_000)) {
    token = await login();
  }

  let lastError = null;

  for (let attempt = 0; attempt < HOST_POOL.length; attempt++) {
    const index = (activeBaseIndex + attempt) % HOST_POOL.length;
    const url = `${HOST_POOL[index]}${path}`;
    const bodyText = body == null ? null : JSON.stringify(body);

    try {
      const response = await fetch(url, {
        method,
        headers: signedHeaders(method, url, bodyText, token),
        body: bodyText || undefined,
      });

      if (response.headers.has("x-user")) {
        try {
          const xUser = JSON.parse(response.headers.get("x-user"));
          const newToken = extractToken(xUser);
          if (newToken) {
            sessionToken = newToken;
            sessionExpiresAt = tokenExpiry(newToken);
          }
        } catch {}
      }

      if (RETRY_STATUS_CODES.has(response.status)) {
        if (response.status === 429) {
          const retryAfter = Number(response.headers.get("retry-after") || 0);
          if (retryAfter > 0) {
            await sleep(Math.min(retryAfter * 1000, 3000));
          }
        }
        continue;
      }

      activeBaseIndex = index;

      if (!response.ok) {
        if (response.status === 401 || response.status === 403) {
          sessionToken = null;
          sessionExpiresAt = 0;
        }
        lastError = new Error(`Upstream HTTP ${response.status}`);
        continue;
      }

      const value = await response.json();
      return value?.data !== undefined ? value.data : value;
    } catch (error) {
      lastError = error;
    }
  }

  if (lastError) throw lastError;
  throw new Error("All upstream hosts exhausted");
}

async function login() {
  const diagnostics = [];

  for (let attempt = 0; attempt < HOST_POOL.length; attempt++) {
    const index = (activeBaseIndex + attempt) % HOST_POOL.length;
    const host = new URL(HOST_POOL[index]).host;
    const url = `${HOST_POOL[index]}/wefeed-mobile-bff/user-api/visitor-login`;

    try {
      const response = await fetch(url, {
        method: "POST",
        headers: signedHeaders("POST", url, "{}", null),
        body: "{}",
      });

      const contentType = response.headers.get("content-type") || "";
      const status = response.status;

      if (!response.ok) {
        diagnostics.push(`${host}:HTTP_${status}`);
        continue;
      }

      let raw;
      try {
        raw = await response.json();
      } catch {
        diagnostics.push(`${host}:INVALID_JSON`);
        continue;
      }

      const payload = raw?.data !== undefined ? raw.data : raw;
      const token = extractToken(payload);

      if (!token) {
        diagnostics.push(`${host}:NO_TOKEN`);
        continue;
      }

      sessionToken = token;
      sessionExpiresAt = tokenExpiry(token);
      activeBaseIndex = index;
      return token;
    } catch (error) {
      diagnostics.push(`${host}:${error?.name || "FETCH_ERROR"}`);
    }
  }

  console.warn("Upstream visitor-login failed", diagnostics);
  throw new Error(
    `Unable to create upstream session (${diagnostics.join(", ") || "no diagnostics"})`
  );
}

async function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

function asStringId(value) {
  if (value === null || value === undefined) return null;
  return String(value);
}

function extractYear(value) {
  if (value === null || value === undefined) return null;
  const match = String(value).match(/\b(19|20)\d{2}\b/);
  return match ? match[0] : null;
}

function catalogItem(subject) {
  if (!subject || typeof subject !== "object") return null;
  const id = asStringId(subject.subjectId ?? subject.id);
  if (!id) return null;

  const stype = Number(subject.subjectType ?? subject.stype ?? 1);
  const cover =
    subject?.cover?.url ??
    subject.coverUrl ??
    subject.poster ??
    subject.pic ??
    null;

  return {
    id: { provider: "movie_box", value: id },
    title: String(subject.title ?? subject.name ?? "Unknown"),
    media_type: stype === 2 ? "series" : "movie",
    year: extractYear(subject.releaseDate ?? subject.year ?? subject.releaseInfo),
    poster_url: typeof cover === "string" ? cover : null,
    season_count: Number.isFinite(Number(subject.season))
      ? Number(subject.season)
      : null,
  };
}

function searchCatalog(payload) {
  const subjects =
    payload?.results?.[0]?.subjects ??
    payload?.list ??
    [];

  return Array.isArray(subjects)
    ? subjects.map(catalogItem).filter(Boolean)
    : [];
}

function homepageCatalog(payload) {
  const groups = Array.isArray(payload?.items)
    ? payload.items
    : Array.isArray(payload)
      ? payload
      : [];

  const items = [];
  const seen = new Set();

  for (const group of groups) {
    const subjects = [];

    for (const item of group?.banner?.banners || []) {
      if (item?.subject) subjects.push(item.subject);
    }
    for (const item of group?.customData?.items || []) {
      if (item?.subject) subjects.push(item.subject);
    }
    for (const item of group?.subjects || []) subjects.push(item);

    for (const subject of subjects) {
      const item = catalogItem(subject);
      if (item && !seen.has(item.id.value)) {
        seen.add(item.id.value);
        items.push(item);
      }
    }
  }
  return items;
}

function detailSubject(payload) {
  const subject = payload?.subject ?? payload?.data?.subject ?? payload;
  if (!subject || typeof subject !== "object") throw new Error("Invalid detail payload");

  const id = asStringId(subject.subjectId ?? subject.id);
  if (!id) throw new Error("Subject not found");

  const stype = Number(subject.subjectType ?? subject.stype ?? 1);
  const seasonsRaw =
    subject?.seasons?.seasons ??
    subject?.seasons ??
    [];

  const seasons = Array.isArray(seasonsRaw)
    ? seasonsRaw.map(s => {
        const number = Number(s?.se ?? s?.season ?? 1);
        let maxEp = Array.isArray(s?.episodeNumbers)
          ? s.episodeNumbers.length
          : Number(s?.maxEp ?? 0);
        return {
          se: number,
          season: number,
          maxEp: maxEp > 0 ? maxEp : 0,
        };
      })
    : [];

  const dubs = Array.isArray(subject?.dubs)
    ? subject.dubs.map(d => ({
        subject_id: asStringId(d?.subjectId ?? d?.id) || "",
        lanName: String(d?.lanName ?? d?.language ?? d?.lang ?? "Unknown"),
        lanCode: String(d?.title ?? d?.name ?? d?.lanName ?? "Unknown"),
      }))
    : [];

  return {
    data: {
      subject: {
        subjectId: id,
        title: String(subject.title ?? "Unknown"),
        subjectType: stype,
        releaseDate: extractYear(subject.releaseDate ?? subject.year),
        description: subject.description ?? subject.intro ?? null,
        tagline: subject.tagline ?? null,
        imdbRatingValue: subject.imdbRatingValue ?? subject.rating ?? null,
        director: subject.director ?? null,
        stars: subject.stars ?? null,
        prints: subject.prints ?? null,
        audios: subject.audios ?? null,
        cover: { url: subject?.cover?.url ?? subject.coverUrl ?? null },
        genre: Array.isArray(subject.genre)
          ? subject.genre
          : Array.isArray(subject.genres)
            ? subject.genres
            : [],
      },
      resource: { seasons },
      dubs,
    },
  };
}

function cleanTitle(raw) {
  let title = String(raw || "").trim();
  if (!title) return "";

  while (title.startsWith("[")) {
    const close = title.indexOf("]");
    if (close < 0) break;
    const rest = title.slice(close + 1).trim();
    if (!rest) break;
    title = rest;
  }

  const bracket = title.indexOf("[");
  if (bracket > 0) title = title.slice(0, bracket).trim();

  const paren = title.indexOf("(");
  if (paren > 0) {
    const inside = title.slice(paren + 1).split(")")[0].trim();
    const isYear = /^\d{4}$/.test(inside) &&
      Number(inside) >= 1900 && Number(inside) <= 2099;
    if (!isYear) title = title.slice(0, paren).trim();
  }

  const tags = [
    "hindi", "tamil", "telugu", "kannada", "malayalam", "bengali",
    "marathi", "punjabi", "gujarati", "urdu", "english", "spanish",
    "french", "german", "italian", "japanese", "korean", "chinese",
    "russian", "portuguese", "turkish", "arabic", "dub", "audio",
    "multi", "season",
  ];

  const dash = title.lastIndexOf(" - ");
  if (dash >= 0 && tags.some(t => title.slice(dash + 3).toLowerCase().includes(t))) {
    title = title.slice(0, dash).trim();
  }

  const seasonMatch = title.match(/\sS\d[\d-]*$/i);
  if (seasonMatch) title = title.slice(0, seasonMatch.index).trim();

  const seasonWord = title.search(/\sseason\s+/i);
  if (seasonWord >= 0) title = title.slice(0, seasonWord).trim();

  title = title.replace(/(?:_| |\.|-)\d{3,4}p$/i, "").trim();
  return title.replace(/[-:_. ]+$/, "").trim();
}

function isNoticeUrl(url) {
  const lower = String(url || "").toLowerCase();
  return lower.includes("1c7de0bd3393702d9191801f15f88f8d") ||
    lower.includes("9a0461bc39da389663bf3dbb17091d3f") ||
    lower.includes("b164fbfb4347792950bdfbfb563d39d9") ||
    lower.includes("/notice.mp4") ||
    lower.includes("notice") ||
    (lower.includes("macdn.aoneroom.com") && lower.includes("/other/"));
}

function resolveDashManifest(signCookie) {
  for (const rawPart of String(signCookie || "").split(";")) {
    const part = rawPart.trim();

    const prefixIndex = part.indexOf("urlprefix=");
    if (prefixIndex >= 0) {
      const raw = part.slice(prefixIndex + "urlprefix=".length).split(":")[0].trim();
      try {
        const normalized = raw.replace(/-/g, "+").replace(/_/g, "/");
        const decoded = Buffer.from(normalized, "base64").toString("utf8");
        const base = decoded.replace(/\*+$/, "").replace(/\/+$/, "");
        if (/^https?:\/\//i.test(base)) return `${base}/index.mpd`;
      } catch {}
    }

    if (part.startsWith("CloudFront-Policy=")) {
      try {
        const raw = part.slice("CloudFront-Policy=".length).trim();
        const normalized = raw.replace(/-/g, "+").replace(/_/g, "=").replace(/~/g, "/");
        const decoded = Buffer.from(normalized, "base64").toString("utf8");
        const policy = JSON.parse(decoded);
        const resource = policy?.Statement?.[0]?.Resource;
        if (typeof resource === "string") {
          const base = resource.replace(/\*+$/, "").replace(/\/+$/, "");
          if (/^https?:\/\//i.test(base)) return `${base}/index.mpd`;
        }
      } catch {}
    }
  }
  return null;
}

function streamReleases(payload, season, episode) {
  // Must run inside a request, not during module initialization.
  const profile = makeClientProfile();
  const data = payload?.data ?? payload;
  const streams = Array.isArray(data?.streams) ? data.streams : [];
  const rawTitle = data?.title || "MovieBox Stream";
  const titlePrefix = cleanTitle(rawTitle);
  const releases = [];

  for (const stream of streams) {
    const id = asStringId(stream?.id);
    const format = stream?.format || "MP4";
    const codec = stream?.codecName ?? stream?.codec ?? null;
    const size = stream?.size != null && !Number.isNaN(Number(stream.size))
      ? Number(stream.size)
      : null;

    const resolutions = String(
      stream?.resolutions ?? data?.displayResolutions ?? "1080,720,480"
    )
      .split(",")
      .map(x => Number(x.trim()))
      .filter(x => Number.isFinite(x) && x > 0)
      .sort((a, b) => b - a);

    const resList = resolutions.length ? [...new Set(resolutions)] : [1080];
    const cookie = stream?.signCookie || "";
    const streamUrl = stream?.url || "";

    const playable =
      resolveDashManifest(cookie) ||
      (!isNoticeUrl(streamUrl) && /^https?:\/\//i.test(streamUrl) ? streamUrl : null);

    if (!playable) continue;

    const highest = resList[0] || 1080;
    const headers = {
      Referer: STREAM_REFERER,
      "User-Agent": profile.userAgent,
    };
    if (cookie) {
      headers.Cookie = cookie
        .replace(/;+$/, "")
        .split(";")
        .map(x => x.trim())
        .filter(Boolean)
        .join("; ");
    }

    for (const res of resList) {
      const quality = `${res}p`;
      const filename = season > 0 && episode > 0
        ? `${titlePrefix} S${String(season).padStart(2, "0")}E${String(episode).padStart(2, "0")} ${quality} ${codec || format}`
        : `${titlePrefix} ${quality} ${codec || format}`;

      let scaledSize = size;
      if (size != null && res < highest) {
        const scale = Math.max(0.15, Math.min(1, Math.pow(res / highest, 1.6)));
        scaledSize = Math.floor(size * scale);
      }

      releases.push({
        filename,
        quality,
        codec,
        language: null,
        size_bytes: scaledSize,
        season: season > 0 ? season : null,
        episode: episode > 0 ? episode : null,
        resource_id: id,
        direct_url: playable,
        headers,
        source: `${quality} ${codec || format}`,
      });
    }
  }

  return releases;
}

function resourceReleases(payload) {
  const list = Array.isArray(payload?.list)
    ? payload.list
    : Array.isArray(payload)
      ? payload
      : [];

  return list
    .map(item => {
      const url = item?.resourceLink ?? item?.url ?? "";
      if (!url || isNoticeUrl(url)) return null;
      return {
        filename: String(item?.fileName ?? item?.title ?? "Unknown Release"),
        quality: item?.resolution != null ? `${item.resolution}`.endsWith("p") ? String(item.resolution) : `${item.resolution}p` : null,
        codec: item?.codecName ?? item?.codec ?? null,
        language: item?.language ?? item?.lanName ?? null,
        size_bytes: item?.size != null && !Number.isNaN(Number(item.size)) ? Number(item.size) : null,
        season: item?.se != null ? Number(item.se) : null,
        episode: item?.ep != null ? Number(item.ep) : null,
        resource_id: asStringId(item?.resourceId ?? item?.id),
        direct_url: url,
        headers: {},
        source: String(item?.uploadBy ?? item?.source ?? "Direct"),
      };
    })
    .filter(Boolean);
}

function sortReleases(releases) {
  return releases.sort((a, b) => {
    const res = q => {
      const s = String(q?.quality || "1080").toLowerCase().replace(/p$/, "");
      if (s === "4k" || s === "uhd") return 2160;
      const n = Number(s);
      return Number.isFinite(n) ? n : 1080;
    };
    return res(b) - res(a) || (Number(b.size_bytes) || 0) - (Number(a.size_bytes) || 0);
  });
}

function base64urlEncode(text) {
  return Buffer.from(text).toString("base64url");
}

function base64urlDecode(text) {
  return Buffer.from(text, "base64url").toString("utf8");
}

async function tokenSignature(payload, secret) {
  return createHmac("sha256", secret).update(payload).digest("base64url");
}

async function createProxyToken(targetUrl, headers, secret) {
  const target = new URL(targetUrl);
  const payload = base64urlEncode(JSON.stringify({
    exp: Math.floor(Date.now() / 1000) + DEFAULT_PROXY_TTL,
    host: target.host,
    headers,
  }));
  const signature = await tokenSignature(payload, secret);
  return `${payload}.${signature}`;
}

async function verifyProxyToken(token, env) {
  const [payload, signature] = String(token || "").split(".");
  if (!payload || !signature) throw new Error("Invalid proxy token");

  const expected = await tokenSignature(payload, requireProxySecret(env));
  if (signature.length !== expected.length) throw new Error("Invalid proxy token");
  if (!timingSafeEqual(signature, expected)) throw new Error("Invalid proxy token");

  const data = JSON.parse(base64urlDecode(payload));
  if (!data.exp || data.exp < Math.floor(Date.now() / 1000)) {
    throw new Error("Proxy token expired");
  }
  return data;
}

function timingSafeEqual(a, b) {
  const aa = Buffer.from(a);
  const bb = Buffer.from(b);
  if (aa.length !== bb.length) return false;
  let diff = 0;
  for (let i = 0; i < aa.length; i++) diff |= aa[i] ^ bb[i];
  return diff === 0;
}

function proxyPath(token, targetUrl) {
  const target = new URL(targetUrl);
  return `/public-proxy/${token}/${target.protocol.replace(":", "")}/${target.host}${target.pathname}${target.search}`;
}

function rewriteManifest(manifest, token, host) {
  const httpsPrefix = `https://${host}/`;
  const httpPrefix = `http://${host}/`;
  return manifest
    .split(httpsPrefix).join(`/public-proxy/${token}/https/${host}/`)
    .split(httpPrefix).join(`/public-proxy/${token}/http/${host}/`);
}

async function publicProxy(request, env, url) {
  const parts = url.pathname.split("/");
  // ["", "public-proxy", token, scheme, host, ...path]
  const token = parts[2];
  const scheme = parts[3];
  const host = parts[4];
  if (!token || !scheme || !host || !["http", "https"].includes(scheme)) {
    return errorResponse("Invalid proxy path", 400);
  }

  let meta;
  try {
    meta = await verifyProxyToken(token, env);
  } catch (error) {
    return errorResponse(error.message, 403);
  }

  if (host !== meta.host) return errorResponse("Target host is not allowed", 403);

  const targetPath = "/" + parts.slice(5).join("/");
  const targetUrl = `${scheme}://${host}${targetPath}${url.search}`;

  const headers = new Headers();
  for (const [name, value] of Object.entries(meta.headers || {})) {
    if (name.toLowerCase() === "user-agent" || host === meta.host) {
      headers.set(name, value);
    }
  }

  const incomingRange = request.headers.get("range");
  if (incomingRange) headers.set("Range", incomingRange);

  let upstream;
  try {
    upstream = await fetch(targetUrl, {
      method: request.method === "HEAD" ? "HEAD" : "GET",
      headers,
    });
  } catch (error) {
    return errorResponse(`Upstream request failed: ${error.message}`, 502);
  }

  const contentType = upstream.headers.get("content-type") || "";
  const isManifest =
    targetUrl.toLowerCase().includes(".mpd") ||
    contentType.includes("dash+xml") ||
    contentType.includes("xml");

  if (upstream.ok && isManifest) {
    const text = await upstream.text();
    const rewritten = rewriteManifest(text, token, host);
    return new Response(rewritten, {
      status: upstream.status,
      headers: {
        "Content-Type": "application/dash+xml",
        ...corsHeaders(),
      },
    });
  }

  const responseHeaders = new Headers();
  for (const name of [
    "content-type", "content-length", "content-range",
    "accept-ranges", "etag", "last-modified",
  ]) {
    const value = upstream.headers.get(name);
    if (value) responseHeaders.set(name, value);
  }
  for (const [key, value] of Object.entries(corsHeaders())) {
    responseHeaders.set(key, value);
  }

  return new Response(upstream.body, {
    status: upstream.status,
    headers: responseHeaders,
  });
}

async function streamEndpoint(subjectId, request, env) {
  const url = new URL(request.url);
  const season = Number(url.searchParams.get("se") || 0);
  const episode = Number(url.searchParams.get("ep") || 0);

  const path = season === 0 && episode === 0
    ? `/wefeed-mobile-bff/subject-api/play-info/v2?subjectId=${encodeURIComponent(subjectId)}`
    : `/wefeed-mobile-bff/subject-api/play-info/v2?subjectId=${encodeURIComponent(subjectId)}&se=${season}&ep=${episode}`;

  const playInfo = await upstreamRequest(path);
  const releases = streamReleases(playInfo, season, episode);

  const resourcePage = episode > 0 ? Math.floor((episode - 1) / 20) + 1 : 1;
  const resourcePath = season === 0 && episode === 0
    ? `/wefeed-mobile-bff/subject-api/resource?subjectId=${encodeURIComponent(subjectId)}&page=${resourcePage}&perPage=20`
    : `/wefeed-mobile-bff/subject-api/resource?subjectId=${encodeURIComponent(subjectId)}&se=${season}&ep=${episode}&page=${resourcePage}&perPage=20`;

  try {
    const resources = await upstreamRequest(resourcePath);
    const fallback = resourceReleases(resources);

    const seen = new Set(releases.map(r => String(r.direct_url).split("?")[0]));
    for (const release of fallback) {
      const key = String(release.direct_url).split("?")[0];
      if (!seen.has(key) &&
          ((season === 0 && episode === 0) ||
           (release.season == null && release.episode == null) ||
           (release.season === season && release.episode === episode))) {
        releases.push(release);
        seen.add(key);
      }
    }
  } catch {}

  sortReleases(releases);

  if (!releases.length) return errorResponse("No playable sources found", 502);

  const secret = requireProxySecret(env);
  const sources = [];
  for (const release of releases) {
    const token = await createProxyToken(release.direct_url, release.headers, secret);
    const proxyUrl = `${url.origin}${proxyPath(token, release.direct_url)}`;

    sources.push({
      url: proxyUrl,
      proxy_url: proxyUrl,
      direct_url: release.direct_url,
      quality: release.quality,
      codec: release.codec,
      language: release.language,
      season: release.season,
      episode: release.episode,
      source: release.source,
    });
  }

  return jsonResponse({ sources });
}


async function handle(request, env) {
  const url = new URL(request.url);

  if (request.method === "OPTIONS") {
    return new Response(null, { status: 204, headers: corsHeaders() });
  }

  try {
    if (url.pathname === "/health") {
      return jsonResponse({
        status: "ok",
        service: "Movie Web API",
        cloudflare: true,
        render: false,
        proxy_configured: Boolean(String(env?.PROXY_SECRET || "").trim()),
      });
    }

    if (url.pathname.startsWith("/public-proxy/")) {
      return publicProxy(request, env, url);
    }

    if (url.pathname === "/search") {
      const query = url.searchParams.get("q") || "";
      if (!query.trim()) return jsonResponse({ items: [] });

      const payload = await upstreamRequest(
        `/wefeed-mobile-bff/subject-api/search/v2`,
        "POST",
        { keyword: query, page: 1, perPage: 15, subjectType: 0 }
      );
      return jsonResponse({ query, items: searchCatalog(payload) });
    }

    if (url.pathname === "/search/suggest") {
      const query = url.searchParams.get("q") || "";
      if (!query.trim()) return jsonResponse({ suggestions: [] });

      const payload = await upstreamRequest(
        `/wefeed-mobile-bff/subject-api/search/v2`,
        "POST",
        { keyword: query, page: 1, perPage: 15, subjectType: 0 }
      );
      const suggestions = searchCatalog(payload).slice(0, 8).map(item => ({
        title: item.title,
        slug: item.id.value,
        subject_id: item.id.value,
        subjectId: item.id.value,
      }));
      return jsonResponse({ suggestions });
    }

    if (url.pathname === "/home") {
      const payload = await upstreamRequest(
        `/wefeed-mobile-bff/tab-operating?page=1&tabId=2&version=`
      );
      return jsonResponse({
        sections: [{ section: "Featured", items: homepageCatalog(payload) }],
      });
    }

    const detailMatch = url.pathname.match(/^\/detail\/([^/]+)$/);
    if (detailMatch) {
      const id = decodeURIComponent(detailMatch[1]);
      let payload = await upstreamRequest(
        `/wefeed-mobile-bff/subject-api/get?subjectId=${encodeURIComponent(id)}`
      );

      const subject = payload?.subject ?? payload;
      if (Number(subject?.subjectType ?? subject?.stype ?? 1) === 2) {
        try {
          const seasons = await upstreamRequest(
            `/wefeed-mobile-bff/subject-api/season-info?subjectId=${encodeURIComponent(id)}`
          );
          if (payload && typeof payload === "object") payload.seasons = seasons;
        } catch {}
      }
      return jsonResponse(detailSubject(payload));
    }

    const streamMatch = url.pathname.match(/^\/api\/stream\/([^/]+)$/);
    if (streamMatch) {
      return streamEndpoint(decodeURIComponent(streamMatch[1]), request, env);
    }

    return errorResponse("Not found", 404);
  } catch (error) {
    console.error(error);
    return errorResponse(error?.message || "Internal error", 502);
  }
}

export default {
  fetch(request, env) {
    return handle(request, env);
  },
};
