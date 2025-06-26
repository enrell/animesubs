"""
Pytest configuration and shared fixtures for AnimeSubs tests.
"""

import tempfile
from pathlib import Path
from unittest.mock import Mock

import pytest


@pytest.fixture
def temp_dir():
    """Create a temporary directory for test files."""
    with tempfile.TemporaryDirectory() as temp_dir:
        yield Path(temp_dir)


@pytest.fixture
def sample_ass_content():
    """Sample ASS subtitle content for testing."""
    return """[Script Info]
Title: Test Subtitle
ScriptType: v4.00+

[V4+ Styles]
Format: Name, Fontname, Fontsize, PrimaryColour, SecondaryColour, OutlineColour, BackColour, Bold, Italic, Underline, StrikeOut, ScaleX, ScaleY, Spacing, Angle, BorderStyle, Outline, Shadow, Alignment, MarginL, MarginR, MarginV, Encoding
Style: Default,Arial,20,&H00FFFFFF,&H000000FF,&H00000000,&H80000000,0,0,0,0,100,100,0,0,1,2,0,2,10,10,10,1

[Events]
Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text
Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello, world!
Dialogue: 0,0:00:04.00,0:00:06.00,Default,,0,0,0,,This is a test.
Dialogue: 0,0:00:07.00,0:00:09.00,Default,,0,0,0,,Konnichiwa, Tanaka-san!
Dialogue: 0,0:00:10.00,0:00:12.00,Default,,0,0,0,,{\\an8}Technical command
Dialogue: 0,0:00:13.00,0:00:15.00,Default,,0,0,0,,m 0 0 l 150 0 150 150 0 150
"""


@pytest.fixture
def sample_subtitle_file(temp_dir, sample_ass_content):
    """Create a sample ASS file for testing."""
    subtitle_path = temp_dir / "test.ass"
    with open(subtitle_path, "w", encoding="utf-8") as f:
        f.write(sample_ass_content)
    return subtitle_path


@pytest.fixture
def mock_genai_client():
    """Mock Google Generative AI client."""
    mock_client = Mock()
    mock_response = Mock()
    mock_response.text = (
        "1: Hello, world!\n2: This is a test.\n3: Konnichiwa, Tanaka-san!"
    )
    mock_client.models.generate_content.return_value = mock_response
    return mock_client


@pytest.fixture
def mock_ffmpeg_success():
    """Mock successful FFmpeg subprocess calls."""
    mock_result = Mock()
    mock_result.returncode = 0
    mock_result.stdout = ""
    mock_result.stderr = ""
    return mock_result


@pytest.fixture
def mock_video_info():
    """Mock video information from FFprobe."""
    return {
        "streams": [
            {"index": 0, "codec_type": "video", "codec_name": "h264"},
            {"index": 1, "codec_type": "audio", "codec_name": "aac"},
            {
                "index": 2,
                "codec_type": "subtitle",
                "codec_name": "ass",
                "tags": {
                    "language": "eng",
                    "title": "English",
                    "NUMBER_OF_FRAMES": "356",
                },
            },
            {
                "index": 3,
                "codec_type": "subtitle",
                "codec_name": "ass",
                "tags": {
                    "language": "jpn",
                    "title": "Japanese",
                    "NUMBER_OF_FRAMES": "400",
                },
            },
        ]
    }


@pytest.fixture
def sample_texts():
    """Sample texts for translation testing."""
    return [
        "Hello, how are you?",
        "I'm fine, thank you.",
        "What's your name?",
        "My name is Tanaka-san.",
        "Nice to meet you!",
    ]


@pytest.fixture
def sample_translated_texts():
    """Sample translated texts (English to Portuguese)."""
    return [
        "Olá, como você está?",
        "Estou bem, obrigado.",
        "Qual é o seu nome?",
        "Meu nome é Tanaka-san.",
        "Prazer em conhecê-lo!",
    ]


@pytest.fixture
def api_key():
    """Test API key."""
    return "test_api_key_12345"


@pytest.fixture
def target_language():
    """Default target language for tests."""
    return "pt-BR"
