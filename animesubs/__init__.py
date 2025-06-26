"""
AnimeSubs - Advanced subtitle translation library
=================================================

A comprehensive Python library for translating subtitle files using AI with
advanced optimizations for batch processing, duplicate removal, and intelligent
text filtering.

Features:
- Batch processing for efficient API usage
- Duplicate text removal and caching
- Intelligent filtering of technical commands and noise
- Support for multiple video formats (MKV, MP4, AVI, etc.)
- FFmpeg integration for video processing
- Multiple language support
- Comprehensive error handling

Example Usage:
    # Single file translation
    from animesubs import SubtitleTranslator

    translator = SubtitleTranslator(api_key="your_api_key")
    translator.translate_subtitle_file("input.ass", "output.ass")

    # Batch video processing
    from animesubs import VideoProcessor, SubtitleTranslator

    translator = SubtitleTranslator(api_key="your_api_key")
    processor = VideoProcessor()

    results = processor.process_directory(
        "/path/to/videos",
        translator,
        target_language="pt-BR"
    )

Installation:
    uv sync

Requirements:
    - pysubs2: For subtitle file processing
    - google-genai: For AI translation
    - python-dotenv: For environment variable management
    - ffmpeg: For video processing (system dependency)

Author: enrell
License: MIT
"""

from .exceptions import (
    AnimeSubsError,
    APIError,
    ConfigurationError,
    FFmpegError,
    FileNotFoundError,
    PermissionError,
    SubtitleExtractionError,
    TranslationError,
    UnsupportedFormatError,
    VideoProcessingError,
)
from .translator import SubtitleTranslator
from .video_processor import VideoProcessor

__version__ = "1.0.0"
__author__ = "enrell"
__email__ = ""
__description__ = "Advanced subtitle translation library with AI integration"

__all__ = [
    "SubtitleTranslator",
    "VideoProcessor",
    "AnimeSubsError",
    "TranslationError",
    "VideoProcessingError",
    "SubtitleExtractionError",
    "FFmpegError",
    "UnsupportedFormatError",
    "APIError",
    "ConfigurationError",
    "FileNotFoundError",
    "PermissionError",
]
