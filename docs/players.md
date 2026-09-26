# Players

MovieBox-TUI delegates playback to external media players (`mpv`, `IINA`, `VLC`, or Android intent players).

## Detection Order

Players are detected in priority order and cached across runs:

- **macOS**: `IINA` → `mpv` → `VLC`
  - Searches `/Applications`, `~/Applications`, Homebrew, MacPorts, Nix, and native CLI tools.
- **Linux**: `mpv` → `VLC`
  - Searches `$PATH`, `~/.local/bin`, Flatpak exports (`io.mpv.Mpv`, `org.videolan.VLC`), and Snap.
- **Windows**: `mpv` → `VLC`
  - Searches `%LOCALAPPDATA%`, `Program Files`, WinGet packages, Scoop shims, Chocolatey, and Windows Registry `App Paths`.
- **Android / Termux**: `Android Intent`
  - Dispatches to external video players via `termux-open` or `am start`.

You can set a default player via `/settings` (Media Player), or override it with the `MOVIEBOX_PLAYER` environment variable.

## Player Invocations

### mpv
```bash
mpv --autofit=WxH --geometry=50%:50% --idle=no --keep-open=no [OPTIONS] <url>
```
- **Tracking**: Injects `moviebox_tracker.lua` to track watch progress and update history every 5 seconds.
- **Headers**: Custom stream headers (`User-Agent`, `Referer`) are passed via `--http-header-fields`.
- **DASH Manifests**: CloudFront cookies are passed to yt-dlp hooks via `--ytdl-raw-options-append`.
- **Subtitles**: Remote subtitles are loaded directly via `--sub-file=<url>`.

### VLC
```bash
vlc --width=W --height=H --play-and-exit --adaptive-logic=highest [OPTIONS] <url>
```
- **Headers**: Mapped to `--http-user-agent` and `--http-referrer`.
- **CloudFront Streams**: CloudFront cookie-authenticated DASH streams route through a local loopback proxy sidecar (`127.0.0.1:<port>`) that injects auth headers server-side.
- **Subtitles**: Remote subtitles are pre-downloaded to temporary storage and passed via `--sub-file=<path>`.

### IINA (macOS)
```bash
iina-cli --keep-running --no-stdin --mpv-autofit=WxH [OPTIONS] <url>
```
- Forwards `--mpv-*` arguments to internal mpv core.
- Falls back to `open -a IINA <url>` if CLI tools are unlinked.

### Android / Termux
```bash
termux-open --chooser --content-type video/* <url>
```
- Opens the system app chooser, delegating playback to installed Android players (VLC, MX Player, Just Player, mpv-android).
- Subtitles are saved to shared storage (`~/storage/downloads/moviebox_subs`) and passed via intent extras.

## Watch History & Resume

- **Position Resumption**: In-progress items launch with `--start=<seconds>` (`mpv`/`IINA`) or `--start-time=<seconds>` (`VLC`).
- **Completion Detection**: Reaching ≥ 90% or stream EOF marks media as completed.
- **State Reconciliation**: `mpv` and `IINA` sync playback progress via background IPC state files, preventing wall-clock drift during pauses or seeks.

## Spawning & Process Safety

- Players launch in detached process groups (`process_group(0)` on Unix, `CREATE_NEW_PROCESS_GROUP` on Windows) so terminal signals do not terminate playback.
- Stderr is drained in the background with a 2-second timeout to prevent pipe deadlocks.
- Clean exits (VLC exit code `1` with empty stderr, or Unix `SIGTERM`) are recognized as normal exits and update watch history without false crash popups.
