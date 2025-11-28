# AnimeSubs

Desktop app (Tauri + Vue 3 + TypeScript) to extract, translate and embed subtitles into videos.

## Features
- Drag-and-drop or pick videos; auto-detect subtitle tracks.
- Translate via pluggable LLM providers (OpenAI-like, Gemini, Ollama/LmStudio).
- Optional embedding of translated subs using mkvmerge (preferred) or ffmpeg.
- Preserves original tracks; skips karaoke/music lines.

## Requirements
- `ffmpeg` available in `PATH` (or set a custom path in Settings).
- `mkvmerge` (MKVToolNix) in `PATH` for best embedding; falls back to ffmpeg if missing.
- Node/Bun for frontend build, Rust toolchain for Tauri.

## Setup
```bash
bun install
# Dev
bun run tauri dev
# Build bundles (all platforms)
bun run tauri build
```

## Platform notes
- **Windows**: App auto-detects common install paths for `ffmpeg.exe` and `mkvmerge.exe`; ensure they’re installed or on `PATH`.
- **macOS/Linux**: Ensure `ffmpeg` and `mkvmerge` are installed (e.g., `brew install ffmpeg mkvtoolnix`).

## License
MIT
