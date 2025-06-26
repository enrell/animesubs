"""
Custom exceptions for AnimeSubs library.

This module defines custom exception classes used throughout the AnimeSubs library
to provide specific error handling for different types of failures that can occur
during subtitle translation and video processing operations.
"""

from typing import List, Optional


class AnimeSubsError(Exception):
    """
    Base exception class for AnimeSubs library.

    All other AnimeSubs exceptions inherit from this base class,
    allowing for catch-all error handling when needed.
    """

    def __init__(self, message: str, cause: Optional[Exception] = None):
        """
        Initialize the exception.

        Args:
            message: Error message describing what went wrong
            cause: Optional underlying exception that caused this error
        """
        super().__init__(message)
        self.message = message
        self.cause = cause

    def __str__(self) -> str:
        """Return string representation of the error."""
        if self.cause:
            return f"{self.message} (caused by: {self.cause})"
        return self.message


class TranslationError(AnimeSubsError):
    """
    Exception raised when translation operations fail.

    This can occur due to API failures, network issues, invalid responses,
    or other problems during the translation process.
    """

    pass


class VideoProcessingError(AnimeSubsError):
    """
    Exception raised when video processing operations fail.

    This includes failures in video analysis, file operations,
    or other video-related processing tasks.
    """

    pass


class SubtitleExtractionError(AnimeSubsError):
    """
    Exception raised when subtitle extraction from video files fails.

    This can occur when FFmpeg fails to extract subtitles, the subtitle
    track is corrupted, or the output file is empty.
    """

    pass


class FFmpegError(AnimeSubsError):
    """
    Exception raised when FFmpeg operations fail.

    This includes failures in video analysis, subtitle extraction,
    subtitle embedding, or other FFmpeg-related operations.
    """

    def __init__(
        self,
        message: str,
        command: Optional[List[str]] = None,
        stderr: Optional[str] = None,
        cause: Optional[Exception] = None,
    ):
        """
        Initialize FFmpeg error with additional context.

        Args:
            message: Error message
            command: FFmpeg command that failed (optional)
            stderr: FFmpeg stderr output (optional)
            cause: Underlying exception (optional)
        """
        super().__init__(message, cause)
        self.command = command
        self.stderr = stderr

    def __str__(self) -> str:
        """Return detailed string representation of FFmpeg error."""
        error_parts = [self.message]

        if self.command:
            error_parts.append(f"Command: {' '.join(self.command)}")

        if self.stderr:
            error_parts.append(f"FFmpeg stderr: {self.stderr}")

        if self.cause:
            error_parts.append(f"Caused by: {self.cause}")

        return "\n".join(error_parts)


class UnsupportedFormatError(AnimeSubsError):
    """
    Exception raised when unsupported video or subtitle format is encountered.

    This occurs when trying to process files with formats that are not
    supported by the library or underlying tools.
    """

    def __init__(
        self,
        message: str,
        file_path: Optional[str] = None,
        format_type: Optional[str] = None,
        cause: Optional[Exception] = None,
    ):
        """
        Initialize unsupported format error.

        Args:
            message: Error message
            file_path: Path to the unsupported file (optional)
            format_type: Type of format (e.g., 'video', 'subtitle') (optional)
            cause: Underlying exception (optional)
        """
        super().__init__(message, cause)
        self.file_path = file_path
        self.format_type = format_type

    def __str__(self) -> str:
        """Return detailed string representation of format error."""
        error_parts = [self.message]

        if self.file_path:
            error_parts.append(f"File: {self.file_path}")

        if self.format_type:
            error_parts.append(f"Format type: {self.format_type}")

        if self.cause:
            error_parts.append(f"Caused by: {self.cause}")

        return "\n".join(error_parts)


class APIError(AnimeSubsError):
    """
    Exception raised when API operations fail.

    This includes failures in authentication, rate limiting,
    quota exceeded, or other API-related issues.
    """

    def __init__(
        self,
        message: str,
        status_code: Optional[int] = None,
        response_body: Optional[str] = None,
        cause: Optional[Exception] = None,
    ):
        """
        Initialize API error.

        Args:
            message: Error message
            status_code: HTTP status code (optional)
            response_body: API response body (optional)
            cause: Underlying exception (optional)
        """
        super().__init__(message, cause)
        self.status_code = status_code
        self.response_body = response_body

    def __str__(self) -> str:
        """Return detailed string representation of API error."""
        error_parts = [self.message]

        if self.status_code:
            error_parts.append(f"Status code: {self.status_code}")

        if self.response_body:
            error_parts.append(f"Response: {self.response_body}")

        if self.cause:
            error_parts.append(f"Caused by: {self.cause}")

        return "\n".join(error_parts)


class ConfigurationError(AnimeSubsError):
    """
    Exception raised when configuration is invalid or missing.

    This includes missing API keys, invalid paths, or other
    configuration-related issues.
    """

    pass


class FileNotFoundError(AnimeSubsError):
    """
    Exception raised when required files are not found.

    This is raised when input files, executables, or other
    required files cannot be located.
    """

    def __init__(
        self,
        message: str,
        file_path: Optional[str] = None,
        cause: Optional[Exception] = None,
    ):
        """
        Initialize file not found error.

        Args:
            message: Error message
            file_path: Path to the missing file (optional)
            cause: Underlying exception (optional)
        """
        super().__init__(message, cause)
        self.file_path = file_path

    def __str__(self) -> str:
        """Return detailed string representation of file not found error."""
        if self.file_path:
            return f"{self.message}: {self.file_path}"
        return self.message


class PermissionError(AnimeSubsError):
    """
    Exception raised when file or directory permissions are insufficient.

    This occurs when the library cannot read input files, write output files,
    or access required directories due to permission restrictions.
    """

    pass
