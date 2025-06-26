"""
Tests for custom exception classes.
"""

from animesubs.exceptions import (
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


class TestExceptions:
    """Test cases for custom exception classes."""

    def test_anime_subs_error_basic(self):
        """Test basic AnimeSubsError functionality."""
        message = "Test error message"
        error = AnimeSubsError(message)

        assert str(error) == message
        assert error.message == message
        assert error.cause is None

    def test_anime_subs_error_with_cause(self):
        """Test AnimeSubsError with underlying cause."""
        message = "Test error message"
        cause = ValueError("Original error")
        error = AnimeSubsError(message, cause=cause)

        assert error.message == message
        assert error.cause == cause
        assert "caused by" in str(error)
        assert "Original error" in str(error)

    def test_translation_error_inheritance(self):
        """Test TranslationError inherits from AnimeSubsError."""
        error = TranslationError("Translation failed")

        assert isinstance(error, AnimeSubsError)
        assert isinstance(error, TranslationError)
        assert str(error) == "Translation failed"

    def test_video_processing_error_inheritance(self):
        """Test VideoProcessingError inherits from AnimeSubsError."""
        error = VideoProcessingError("Video processing failed")

        assert isinstance(error, AnimeSubsError)
        assert isinstance(error, VideoProcessingError)
        assert str(error) == "Video processing failed"

    def test_subtitle_extraction_error_inheritance(self):
        """Test SubtitleExtractionError inherits from AnimeSubsError."""
        error = SubtitleExtractionError("Subtitle extraction failed")

        assert isinstance(error, AnimeSubsError)
        assert isinstance(error, SubtitleExtractionError)
        assert str(error) == "Subtitle extraction failed"

    def test_ffmpeg_error_basic(self):
        """Test basic FFmpegError functionality."""
        message = "FFmpeg operation failed"
        error = FFmpegError(message)

        assert isinstance(error, AnimeSubsError)
        assert str(error) == message
        assert error.command is None
        assert error.stderr is None

    def test_ffmpeg_error_with_details(self):
        """Test FFmpegError with command and stderr details."""
        message = "FFmpeg operation failed"
        command = ["ffmpeg", "-i", "input.mkv", "output.mkv"]
        stderr = "Error: invalid codec"

        error = FFmpegError(message, command=command, stderr=stderr)

        assert error.message == message
        assert error.command == command
        assert error.stderr == stderr

        error_str = str(error)
        assert message in error_str
        assert "ffmpeg -i input.mkv output.mkv" in error_str
        assert "Error: invalid codec" in error_str

    def test_unsupported_format_error_basic(self):
        """Test basic UnsupportedFormatError functionality."""
        message = "Unsupported format"
        error = UnsupportedFormatError(message)

        assert isinstance(error, AnimeSubsError)
        assert str(error) == message
        assert error.file_path is None
        assert error.format_type is None

    def test_unsupported_format_error_with_details(self):
        """Test UnsupportedFormatError with file and format details."""
        message = "Unsupported format"
        file_path = "/path/to/file.xyz"
        format_type = "video"

        error = UnsupportedFormatError(
            message, file_path=file_path, format_type=format_type
        )

        assert error.file_path == file_path
        assert error.format_type == format_type

        error_str = str(error)
        assert message in error_str
        assert file_path in error_str
        assert format_type in error_str

    def test_api_error_basic(self):
        """Test basic APIError functionality."""
        message = "API request failed"
        error = APIError(message)

        assert isinstance(error, AnimeSubsError)
        assert str(error) == message
        assert error.status_code is None
        assert error.response_body is None

    def test_api_error_with_details(self):
        """Test APIError with HTTP details."""
        message = "API request failed"
        status_code = 429
        response_body = '{"error": "Rate limit exceeded"}'

        error = APIError(message, status_code=status_code, response_body=response_body)

        assert error.status_code == status_code
        assert error.response_body == response_body

        error_str = str(error)
        assert message in error_str
        assert "429" in error_str
        assert "Rate limit exceeded" in error_str

    def test_configuration_error_inheritance(self):
        """Test ConfigurationError inherits from AnimeSubsError."""
        error = ConfigurationError("Invalid configuration")

        assert isinstance(error, AnimeSubsError)
        assert isinstance(error, ConfigurationError)
        assert str(error) == "Invalid configuration"

    def test_file_not_found_error_basic(self):
        """Test basic FileNotFoundError functionality."""
        message = "File not found"
        error = FileNotFoundError(message)

        assert isinstance(error, AnimeSubsError)
        assert str(error) == message
        assert error.file_path is None

    def test_file_not_found_error_with_path(self):
        """Test FileNotFoundError with file path."""
        message = "File not found"
        file_path = "/path/to/missing/file.txt"

        error = FileNotFoundError(message, file_path=file_path)

        assert error.file_path == file_path
        assert file_path in str(error)

    def test_permission_error_inheritance(self):
        """Test PermissionError inherits from AnimeSubsError."""
        error = PermissionError("Permission denied")

        assert isinstance(error, AnimeSubsError)
        assert isinstance(error, PermissionError)
        assert str(error) == "Permission denied"

    def test_exception_chaining(self):
        """Test exception chaining with multiple causes."""
        original_error = ValueError("Original error")
        intermediate_error = TranslationError(
            "Translation failed", cause=original_error
        )
        final_error = VideoProcessingError(
            "Video processing failed", cause=intermediate_error
        )

        assert final_error.cause == intermediate_error
        assert intermediate_error.cause == original_error

        final_str = str(final_error)
        assert "Video processing failed" in final_str
        assert "Translation failed" in final_str
