<div align="center">

# Introduction

**Terminal interface to find, download, and stream movies, TV shows, and live TV using local media players.**

[ English ](../README.md) • [ বাংলা ](../README_BN.md) • [ हिन्दी ](../README_HI.md) • [ Español ](../README_ES.md)

[![CI](https://img.shields.io/github/actions/workflow/status/mesamirh/MovieBox-Tui/ci.yml?branch=main&label=CI&logo=github&style=flat)](https://github.com/mesamirh/MovieBox-Tui/actions/workflows/ci.yml)
[![crates.io](https://img.shields.io/crates/v/moviebox-tui.svg?logo=rust&style=flat)](https://crates.io/crates/moviebox-tui)
[![License](https://img.shields.io/badge/license-MIT%2FApache--2.0-blue.svg?style=flat)](../README.md#license)
[![Telegram](https://img.shields.io/badge/Telegram-Channel-2CA5E0?style=flat&logo=telegram&logoColor=white)](https://t.me/getfromme)
[![Support](https://img.shields.io/badge/Support-Crypto-F7931A?style=flat&logo=bitcoin&logoColor=white)](../README.md#optional-support)
</div>

[moviebox-tui-walkthrough.webm](https://github.com/user-attachments/assets/7554a7e5-6ff5-49ec-9d87-f821ea99950e)

## Features

- **On Demand Streaming**: Stream movies, TV series, anime, and Asian dramas across multiple native providers and community Stremio addons.
- **Live TV & IPTV**: Import custom M3U playlist URLs to search channels, browse categories, and stream live television.
- **Quality & Resolution Selection**: Choose preferred stream resolutions (`1080p`, `720p`, `480p`) directly from the details screen.
- **Hardware Accelerated Playback**: Plays directly in your preferred local media player with automatic authentication and cookie forwarding.
- **Multi Segment Downloader**: Download individual episodes or full seasons concurrently with HTTP range pause and resume support.
- **Automatic Subtitles**: Searches and synchronizes subtitles in your preferred language automatically.
- **Interactive Terminal UI**: Full keyboard and mouse support with vim navigation and command auto suggestions.
- **Visual Posters & Themes**: Renders cover art directly in your terminal, with 9 built-in themes and automatic light and dark detection.
- **Library & Progress Tracking**: Star favorites, track watch history, and resume playback where you left off.
- **Cross Platform & Private**: Runs natively on macOS, Linux, Windows, and Android (Termux) with zero telemetry or data collection.

## Prerequisites

### Supported Media Players

Requires at least one supported media player installed on your system:

- **Desktop (macOS, Linux, Windows):** [mpv](https://mpv.io/), [VLC](https://www.videolan.org/), or [IINA](https://iina.io/) *(macOS)*.
- **Android (Termux):** Any external video player ([VLC](https://play.google.com/store/apps/details?id=org.videolan.vlc), Just Player, or MX Player).

### Terminal Graphics (Posters)

Poster rendering automatically adapts to your terminal environment:

- **Graphics-capable terminals:** Displays high-resolution movie and series posters natively.
- **Standard terminals:** Displays clean, structured text placeholders automatically.

### Optional Dependencies

- **`yt-dlp` & `ffmpeg`:** Required only when downloading DASH streams from the MovieBox provider. All other providers download directly through the built-in HTTP engine.
---

## Documentation Directory Map

### Getting Started

| Guide | Description |
| :--- | :--- |
| [Installation](installation.md) | Platform installation instructions, package managers, and binary verification |
| [Keyboard & Controls](controls.md) | Complete keybindings, vim navigation, text editing, and slash commands |
| [Configuration Guide](config.md) | `config.json` schema, settings hub options, and environment variables |

### Features & Modes

| Guide | Description |
| :--- | :--- |
| [Content Providers](providers.md) | Built-in providers, scrapers, stream extractors, and authentication headers |
| [Hardware Players](players.md) | Media player detection, launch flags, stream headers, and watch tracking |
| [Batch Downloads](downloads.md) | Multi-segment download engine, range resume, and folder layout |
| [Stremio Addons](addons-mode.md) | Addon manifest installation, catalog browsing, and stream resolution |
| [Live TV & IPTV](tv-mode.md) | M3U playlist manager, channel parsing, and live stream playback |

### Architecture & Internals

| Guide | Description |
| :--- | :--- |
| [System Architecture](architecture.md) | Subsystem diagrams, async event loop, and task cancellation |
| [Module Breakdown](modules.md) | Crate structure, module responsibilities, and call boundaries |
| [Caching Strategy](cache.md) | Binary disk caching, TTL policies, and LRU memory management |
| [Logging System](logging.md) | File logging, log rotation, and tracing diagnostics |
| [Cross-Platform Operations](cross-platform.md) | Platform compatibility matrix across macOS, Linux, Windows, and Termux |

### Reference & Maintenance

| Guide | Description |
| :--- | :--- |
| [Testing Suite](testing.md) | Unit tests, integration tests, and verification gates |
| [Debugging Guide](debugging.md) | Troubleshooting common issues, terminal rendering, and player errors |
| [Release Checklist](release-checklist.md) | Pre-release validation, binary packaging, and deployment workflow |
| [Known Issues](known-issues.md) | Tracked limitations, terminal quirks, and workarounds |
| [Contributing Guide](contributing.md) | Contribution guidelines, code standards, and PR process |
| [Changelog](changelog.md) | Complete release history and unreleased changes |
