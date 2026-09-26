# Cross-Platform Support

MovieBox-TUI runs on **macOS**, **Linux**, **Windows**, and **Android (Termux)**.

## Supported Operating Systems

### macOS
- **Media Players**: `IINA` (CLI + app bundle), `mpv`, and `VLC`.
- **Binaries**: Shipped as universal binaries supporting both Apple Silicon (`arm64`) and Intel (`x86_64`).

### Linux
- **Media Players**: Native `mpv`, `VLC`, or Flatpaks (`flatpak run io.mpv.Mpv`).
- **Binaries**: Static Musl executables compatible with glibc and musl systems across x86_64 and aarch64.

### Windows
- **Media Players**: Discovers `mpv.exe` and `vlc.exe` via WinGet, Scoop, Program Files, and the Windows Registry.
- **Process Safety**: Processes launch with `CREATE_NO_WINDOW` and independent process groups so terminal signals do not terminate playback or downloads.
- **Path Compatibility**: Supports Windows drive letters (`C:\...`), backslashes (`\`), forward slashes (`/`), and UNC paths.

### Android (Termux)
- **Media Players**: Uses Android intent dispatchers (`termux-open` / `am start`) to stream directly to external video players (VLC for Android, Just Player, MX Player, MPV Android).
- **Prerequisites**: Run `pkg install -y termux-tools` in Termux.
- **Subtitles**: Subtitles are saved to shared storage (`~/storage/downloads/moviebox_subs`) for player access.
- **DNS**: Built-in DNS resolver queries public resolvers (Cloudflare, Google, Quad9) without requiring root or Android JNI.

## Terminal Compatibility

MovieBox-TUI automatically adapts to your terminal emulator:

- **Images & Posters**: Automatically detects Kitty graphics, Sixel, and iTerm2 protocols (Ghostty, Kitty, WezTerm, iTerm2, foot, Alacritty). Terminals without image support display clean text containers. Disable with `MOVIEBOX_NO_IMAGE=1`.
- **Colors & Themes**: Auto-detects 24-bit TrueColor, 256-color palettes, and high-contrast monochrome mode (`NO_COLOR=1`).
- **Keyboard Navigation**: Uses the Kitty keyboard protocol on supported emulators. Mobile Termux keyboards use standard ANSI input to avoid character garbage.
- **Window Titles**: Automatically updates terminal window titles based on current title and screen mode.
