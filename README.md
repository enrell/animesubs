# AnimeSubs (beta)

Advanced anime subtitle translation library with AI integration for batch processing video files.

## Features

- **Batch Processing**: Translate entire directories of video files automatically
- **Anime-Optimized Translation**: Preserves Japanese honorifics and cultural terms
- **Smart Filtering**: Intelligent detection and removal of technical commands and noise
- **Duplicate Optimization**: Remove duplicate texts to minimize API calls
- **Multi-Format Support**: Works with various video and subtitle formats
- **FFmpeg Integration**: Automatic subtitle extraction and embedding
- **Multiple Languages**: Support for various target languages
- **Backup Safety**: Creates backup files before modification
- **Progress Tracking**: Detailed progress reporting and statistics

## Installation

### Prerequisites

- Python 3.9+
- FFmpeg (for video processing)
- [uv](https://docs.astral.sh/uv/) (recommended package manager)

### Using uv (Recommended)

```bash
# Install uv if you haven't already
curl -LsSf https://astral.sh/uv/install.sh | sh

# Clone the repository
git clone https://github.com/enrell/animesubs.git
cd animesubs

# Create virtual environment and install dependencies
uv sync

# Install development dependencies (optional)
uv sync --extra dev
```
### Environment Setup

Create a `.env` file with your Google Gemini API key:

```bash
GEMINI_API_KEY=your_api_key_here
```

Get your API key from [Google AI Studio](https://makersuite.google.com/app/apikey).

## Usage

### Single File Translation

Translate a single subtitle file:

```bash
# Using uv
uv run python main.py input.ass -l pt-BR

# Or if installed globally
python main.py input.ass -l pt-BR
```

### Batch Directory Processing

Process all video files in a directory:

```bash
# Using uv
uv run python batch_translate.py /path/to/video/directory

# Or if installed globally
python batch_translate.py /path/to/video/directory
```

### Library Usage

```python
from animesubs import SubtitleTranslator, VideoProcessor

# Single file translation
translator = SubtitleTranslator(api_key="your_api_key")
translator.translate_subtitle_file("input.ass", "output.ass")

# Batch video processing
processor = VideoProcessor()
results = processor.process_directory(
    "/path/to/videos", 
    translator, 
    target_language="pt-BR"
)
```

## Anime-Specific Translation Features

AnimeSubs is specifically designed for anime content with special handling for Japanese honorifics and cultural terms.

## How It Works

1.  **Video Analysis**: Scans video files for existing subtitle tracks.
2.  **Language Detection**: Identifies videos missing target language subtitles.
3.  **Best Track Selection**: Selects the best subtitle track (prioritizes English/Spanish).
4.  **Intelligent Extraction**: Extracts subtitles using FFmpeg.
5.  **Anime-Optimized Translation**: Translates using Google Gemini AI with anime-specific prompts.
6.  **Seamless Integration**: Embeds translated subtitles back into video files.
7.  **Metadata Update**: Updates language metadata for new subtitle tracks.

## Development

### Running Tests

```bash
uv run pytest
```

### Code Quality

```bash
uv run ruff format .
uv run isort .
uv run flake8
uv run mypy animesubs
```

### Project Structure

```
animesubs/
├── animesubs/           # Main library package
│   ├── __init__.py
│   ├── translator.py
│   ├── video_processor.py
│   └── exceptions.py
├── tests/              # Test suite
│   ├── test_translator.py
│   ├── test_video_processor.py
│   ├── test_exceptions.py
│   └── conftest.py
├── main.py            # Single file translation script
├── batch_translate.py # Batch processing script
├── pyproject.toml     # Project configuration
└── README.md          # This file
```

## Supported Formats

- **Video**: MKV, MP4, AVI, MOV, WMV, FLV, WebM
- **Subtitle**: ASS/SSA, SRT

## Language Support

Supports translation to various languages while preserving anime elements.

## Configuration Options

Key options include `batch_size` and `target_language`.

## Error Handling

Includes comprehensive error handling for various issues.

## Performance

Optimized through duplicate removal and batch processing.

## Safety Features

Includes backup files, dry run mode, and skipping existing translations.

## Contributing

See CONTRIBUTING.md (if it exists) or the contributing section in this README.

## License

MIT License - see LICENSE file for details.

## Author

Created by **enrell**

GitHub: https://github.com/enrell/animesubs

## Support

For issues and feature requests, please visit:
https://github.com/enrell/animesubs/issues

## Acknowledgments

- Built with [Google Gemini AI](https://ai.google.dev/) for translation
- Uses [FFmpeg](https://ffmpeg.org/) for video processing
- Powered by [uv](https://docs.astral.sh/uv/) for dependency management