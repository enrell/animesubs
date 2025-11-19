from typing import List, Optional


class AnimeSubsError(Exception):
    def __init__(self, message: str, cause: Optional[Exception] = None):
        super().__init__(message)
        self.message = message
        self.cause = cause

    def __str__(self) -> str:
        if self.cause:
            return f"{self.message} (caused by: {self.cause})"
        return self.message


class TranslationError(AnimeSubsError):
    pass


class VideoProcessingError(AnimeSubsError):
    pass


class SubtitleExtractionError(VideoProcessingError):
    pass


class FFmpegError(VideoProcessingError):
    pass


class UnsupportedFormatError(VideoProcessingError):
    pass
