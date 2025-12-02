# AnimeSubs

Desktop app to extract, translate and embed subtitles into videos.

<img width="1407" height="809" alt="Untitled" src="https://github.com/user-attachments/assets/9ac2d975-70ab-4c63-ab8d-78391a88e1ad" />

![License: GPL-3.0](https://img.shields.io/badge/License-GPL--3.0-blue.svg)

## Features

### Video Management
- Drag-and-drop or file picker for videos
- Batch folder scanning for video files
- Supports MKV, MP4, WebM, AVI, MOV, WMV, FLV, M4V formats
- Auto-detect embedded subtitle tracks with language and codec info

### Translation
- **Multiple LLM Providers**: OpenAI, Google Gemini, Ollama, LM Studio, OpenRouter
- **Translation Styles**: Natural, Literal, Localized, Formal, Casual, Honorifics-preserved
- **Smart Filtering**: Automatically skips OP/ED songs, karaoke, signs, and music lines
- Batch processing with configurable batch size and request delay
- Preserves ASS formatting and styles during translation

### Subtitle Formats
- Full support for SRT, ASS/SSA, and WebVTT
- Automatic character encoding detection (UTF-8, Shift-JIS, etc.)
- Preserves original formatting and timing

### Embedding
- Embed translated subtitles back into video files
- Uses mkvmerge (preferred) or ffmpeg for embedding
- Automatic duplicate track removal
- Set translated track as default

### Backup System
- Automatic backup of original subtitles before modification
- Restore from backups at any time
- Per-video backup management

## Requirements
- `ffmpeg` available in `PATH` (or set a custom path in Settings)
- `mkvmerge` (MKVToolNix) in `PATH` for best embedding; falls back to ffmpeg if missing
- Node/Bun for frontend build, Rust toolchain for Tauri

## Installation

### From Releases
Download pre-built binaries from the [Releases](https://github.com/enrell/animesubs/releases) page.

### Build from Source
```bash
# Install dependencies
bun install

# Development mode
bun run tauri dev

# Build for production
bun run tauri build
```

## Usage

### Quick Start
1. Open AnimeSubs
2. Drag-and-drop video files or use the file picker
3. Configure your LLM provider in Settings (gear icon)
4. Select target language and translation style
5. Click "Start Translation"

### API Configuration
Go to Settings and configure your preferred provider:

| Provider | Endpoint | API Key Required |
|----------|----------|------------------|
| OpenAI | `https://api.openai.com/v1` | Yes |
| Gemini | `https://generativelanguage.googleapis.com/v1beta/openai` | Yes |
| Ollama | `http://localhost:11434/v1` | No |
| LM Studio | `http://localhost:1234/v1` | No |
| OpenRouter | `https://openrouter.ai/api/v1` | Yes |

### Translation Options
- **Target Language**: Choose from 15+ supported languages
- **Translation Style**: 
  - *Natural* - Fluent, native-sounding translations
  - *Literal* - Word-for-word accuracy
  - *Localized* - Cultural adaptation
  - *Formal/Casual* - Tone adjustment
  - *Honorifics* - Preserve Japanese honorifics (-san, -kun, etc.)
- **Batch Size**: Lines per API call (default: 100)
- **Concurrency**: Number of parallel API requests (1-10)
- **Request Delay**: Milliseconds between API calls to avoid rate limits

### Advanced Features
- **Embed Subtitles**: Mux translated subs directly into video
- **Custom FFmpeg Path**: Use non-standard FFmpeg installation
- **Subtitle Format**: Output as SRT, ASS, or WebVTT

## Platform Notes

### Windows
- Auto-detects FFmpeg and MKVToolNix in common install paths:
  - `C:\Program Files\FFmpeg\bin\`
  - `C:\Program Files\MKVToolNix\`
- Download FFmpeg: https://ffmpeg.org/download.html#build-windows
- Download MKVToolNix: https://mkvtoolnix.download/downloads.html#windows

### macOS
- Install via Homebrew (recommended):
  ```bash
  brew install ffmpeg mkvtoolnix
  ```
- Auto-detects binaries in:
  - `/opt/homebrew/bin/` (Apple Silicon)
  - `/usr/local/bin/` (Intel Mac)
  - `/Applications/MKVToolNix.app/Contents/MacOS/`

### Linux
- Install via package manager:
  ```bash
  # Ubuntu/Debian
  sudo apt install ffmpeg mkvtoolnix
  
  # Fedora
  sudo dnf install ffmpeg mkvtoolnix
  
  # Arch Linux
  sudo pacman -S ffmpeg mkvtoolnix-cli
  ```

## Troubleshooting

### FFmpeg not found
1. Check if FFmpeg is in your system PATH: `ffmpeg -version`
2. If installed but not detected, set the custom path in Settings > Output > FFmpeg Path

### Translation errors
- **Rate limit**: Increase "Request Delay" in Advanced Options
- **Timeout**: Reduce "Batch Size" or "Concurrency"
- **API errors**: Verify your API key and endpoint in Settings

### macOS security warning
If macOS blocks the app, go to System Preferences > Security & Privacy and allow it.

## License
[GPL-3.0](LICENSE)
