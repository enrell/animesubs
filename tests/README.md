# Tests for AnimeSubs library

This directory contains test files for the AnimeSubs library using pytest.

## Running Tests

To run all tests:
```bash
uv run pytest
```

To run tests with coverage:
```bash
uv run pytest --cov=animesubs --cov-report=html
```

To run specific test files:
```bash
uv run pytest tests/test_translator.py
uv run pytest tests/test_video_processor.py
```

## Test Structure

- `test_translator.py`: Tests for the SubtitleTranslator class
- `test_video_processor.py`: Tests for the VideoProcessor class  
- `test_exceptions.py`: Tests for custom exception classes
- `conftest.py`: Common pytest fixtures and configuration
- `fixtures/`: Test data files (subtitle samples, etc.)
