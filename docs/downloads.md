# Downloader

MovieBox-TUI includes a multi-segment HTTP chunked downloader supporting pause, resume, and authentication header forwarding.

## Overview

- **Storage Location**: Defaults to `~/Downloads/MovieBox-TUI/`. Configurable via `/settings` (General → Download Folder).
- **Multi-Segment Engine**: Files are partitioned into concurrent byte ranges using HTTP RFC 7233 `Range: bytes=X-Y` requests.
- **Single-Stream Fallback**: If an upstream server or CDN does not support range requests (returns HTTP `200 OK` instead of `206 Partial Content`), the engine falls back to single-stream downloading without failing.

## File Lifecycle & State Files

During download, files are saved with temporary extensions:

```text
destination.mp4.part       # In-progress byte buffer
destination.mp4.part.json  # Download state (ETag, Last-Modified, total size, segment offsets)
destination.mp4.assembling # Sequential segment assembly
destination.mp4            # Final verified output
```

Upon completion, temporary sidecars are verified and renamed atomically to the target filename.

## Download Controls

- Press **`d`** on any stream in the Details screen to start downloading immediately.
- Press **`x`** or **`X`** during an active download to cancel or pause the transfer. Partial `.part` data is preserved on disk for resumption.
- Downloads run cooperatively in the background, allowing you to browse or search without interrupting transfers.

## Header Forwarding

Authenticated streams (such as MovieBox DASH manifests or 4KHDHub mirrors) automatically forward required headers (`User-Agent`, `Referer`, signed CloudFront cookies) to download workers, ensuring CDN transfers complete without `403 Forbidden` errors.

## DASH Streams (`yt-dlp`)

MovieBox DASH streams require `yt-dlp` and `ffmpeg` to download and mux adaptive video/audio representations:

- **Windows**: `winget install yt-dlp.yt-dlp Gyan.FFmpeg`
- **macOS**: `brew install yt-dlp ffmpeg`
- **Android (Termux)**: `pkg install yt-dlp ffmpeg`
- **Linux**: Install `yt-dlp` and `ffmpeg` via system package manager.

All other providers (4KHDHub, BDIX, DhakaFlix, CircleFTP, TV mode) download directly through the internal HTTP engine.
