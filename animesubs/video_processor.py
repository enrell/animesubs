#!/usr/bin/env python3
"""
Video processor module for AnimeSubs library.

This module provides the VideoProcessor class for processing video files,
extracting subtitles, and managing subtitle tracks using FFmpeg.
"""

import json
import os
import subprocess
import tempfile
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any, Dict, List, Optional, Tuple

from .exceptions import (
    FFmpegError,
    SubtitleExtractionError,
    UnsupportedFormatError,
    VideoProcessingError,
)
from .translator import SubtitleTranslator


class VideoProcessor:
    """
    Advanced video processor for subtitle extraction, translation, and embedding.

    Features:
    - Extract subtitle tracks from video files
    - Analyze subtitle language and content
    - Translate subtitles using AI
    - Embed translated subtitles back into videos
    - Support for multiple video formats (MKV, MP4, AVI, etc.)
    - FFmpeg integration for video operations
    """

    # Language preference order for subtitle extraction
    PREFERRED_LANGUAGES = ["eng", "en", "spa", "es", "jpn", "ja"]

    # Language code mappings
    LANGUAGE_CODES = {
        "pt-BR": "por",
        "pt": "por",
        "en": "eng",
        "es": "spa",
        "ja": "jpn",
        "zh": "chi",
        "fr": "fre",
        "de": "ger",
        "it": "ita",
        "ru": "rus",
        "ko": "kor",
    }

    def __init__(
        self,
        ffmpeg_path: str = "ffmpeg",
        ffprobe_path: str = "ffprobe",
        concurrence: int = 1,
    ):
        """
        Initialize the video processor.

        Args:
            ffmpeg_path: Path to FFmpeg executable
            ffprobe_path: Path to FFprobe executable

        Raises:
            VideoProcessingError: If FFmpeg/FFprobe is not available
        """
        self.ffmpeg_path = ffmpeg_path
        self.ffprobe_path = ffprobe_path
        self.concurrence = concurrence

        # Verify FFmpeg availability
        self._verify_ffmpeg()

    def _verify_ffmpeg(self) -> None:
        """
        Verify that FFmpeg and FFprobe are available.

        Raises:
            VideoProcessingError: If FFmpeg/FFprobe is not available
        """
        try:
            subprocess.run(
                [self.ffmpeg_path, "-version"], capture_output=True, check=True
            )
            subprocess.run(
                [self.ffprobe_path, "-version"], capture_output=True, check=True
            )
        except (subprocess.CalledProcessError, FileNotFoundError) as e:
            raise VideoProcessingError(f"FFmpeg not available: {e}")

    def get_video_info(self, video_path: str) -> Dict[str, Any]:
        """
        Get detailed information about a video file.

        Args:
            video_path: Path to video file

        Returns:
            Dictionary containing video information

        Raises:
            VideoProcessingError: If failed to get video info
        """
        try:
            cmd = [
                self.ffprobe_path,
                "-v",
                "quiet",
                "-print_format",
                "json",
                "-show_format",
                "-show_streams",
                video_path,
            ]

            result = subprocess.run(cmd, capture_output=True, text=True, check=True)
            return json.loads(result.stdout)

        except (subprocess.CalledProcessError, json.JSONDecodeError) as e:
            raise VideoProcessingError(f"Failed to get video info: {e}")

    def get_subtitle_tracks(self, video_path: str) -> List[Dict[str, Any]]:
        """
        Get information about subtitle tracks in a video file.

        Args:
            video_path: Path to video file

        Returns:
            List of subtitle track information dictionaries

        Raises:
            VideoProcessingError: If failed to get subtitle tracks
        """
        video_info = self.get_video_info(video_path)

        subtitle_tracks = []
        for stream in video_info.get("streams", []):
            if stream.get("codec_type") == "subtitle":
                track_info = {
                    "index": stream.get("index"),
                    "codec_name": stream.get("codec_name"),
                    "language": stream.get("tags", {}).get("language", "unknown"),
                    "title": stream.get("tags", {}).get("title", ""),
                    "disposition": stream.get("disposition", {}),
                    "frame_count": stream.get("tags", {}).get("NUMBER_OF_FRAMES", "0"),
                }
                subtitle_tracks.append(track_info)

        return subtitle_tracks

    def has_target_language(self, video_path: str, target_language: str) -> bool:
        """
        Check if video already has subtitles in target language.

        Args:
            video_path: Path to video file
            target_language: Target language code (e.g., 'pt-BR')

        Returns:
            True if target language subtitles exist, False otherwise
        """
        subtitle_tracks = self.get_subtitle_tracks(video_path)
        target_code = self.LANGUAGE_CODES.get(target_language, target_language)

        for track in subtitle_tracks:
            track_lang = track["language"].lower()
            if track_lang == target_code.lower() or track_lang.startswith(
                target_code.lower()[:2]
            ):
                return True

        return False

    def select_best_subtitle_track(
        self, subtitle_tracks: List[Dict[str, Any]]
    ) -> Optional[Dict[str, Any]]:
        """
        Select the best subtitle track for translation based on language preference.

        Args:
            subtitle_tracks: List of subtitle track information

        Returns:
            Best subtitle track info or None if no suitable track found
        """
        if not subtitle_tracks:
            return None

        # Sort tracks by preference
        def get_language_priority(track):
            lang = track["language"].lower()
            for i, preferred in enumerate(self.PREFERRED_LANGUAGES):
                if lang.startswith(preferred.lower()):
                    return i
            return len(self.PREFERRED_LANGUAGES)

        # Filter out subtitle tracks that are likely non-dialogue
        dialogue_tracks = []
        for track in subtitle_tracks:
            title = track.get("title", "").lower()
            # Skip signs/songs subtitles
            if any(
                keyword in title
                for keyword in ["sign", "song", "opening", "ending", "credit"]
            ):
                continue
            dialogue_tracks.append(track)

        # Use all tracks if no dialogue tracks found
        if not dialogue_tracks:
            dialogue_tracks = subtitle_tracks

        # Sort by language preference and frame count (more frames = more dialogue)
        dialogue_tracks.sort(
            key=lambda x: (get_language_priority(x), -int(x.get("frame_count", "0")))
        )

        return dialogue_tracks[0] if dialogue_tracks else None

    def extract_subtitle(
        self, video_path: str, subtitle_index: int, output_path: str
    ) -> bool:
        """
        Extract a subtitle track from video file.

        Args:
            video_path: Path to video file
            subtitle_index: Index of subtitle track to extract
            output_path: Path for extracted subtitle file

        Returns:
            True if successful, False otherwise

        Raises:
            SubtitleExtractionError: If extraction fails
        """
        try:
            cmd = [
                self.ffmpeg_path,
                "-i",
                video_path,
                "-map",
                f"0:s:{subtitle_index}",
                "-c",
                "copy",
                "-y",  # Overwrite output file
                output_path,
            ]

            result = subprocess.run(cmd, capture_output=True, text=True)

            if result.returncode != 0:
                raise SubtitleExtractionError(f"FFmpeg error: {result.stderr}")

            # Verify output file exists and has content
            if not os.path.exists(output_path) or os.path.getsize(output_path) == 0:
                raise SubtitleExtractionError(
                    "Extracted subtitle file is empty or doesn't exist"
                )

            return True

        except Exception as e:
            raise SubtitleExtractionError(f"Failed to extract subtitle: {e}")

    def embed_subtitle(
        self,
        video_path: str,
        subtitle_path: str,
        target_language: str,
        output_path: str,
    ) -> bool:
        """
        Embed translated subtitle into video file.

        Args:
            video_path: Path to original video file
            subtitle_path: Path to translated subtitle file
            target_language: Target language code
            output_path: Path for output video file

        Returns:
            True if successful, False otherwise

        Raises:
            VideoProcessingError: If embedding fails
        """
        try:
            # Get language code for metadata
            lang_code = self.LANGUAGE_CODES.get(target_language, target_language)

            cmd = [
                self.ffmpeg_path,
                "-i",
                video_path,
                "-i",
                subtitle_path,
                "-c",
                "copy",
                "-map",
                "0",  # Copy all streams from first input
                "-map",
                "1",  # Add subtitle from second input
                f"-metadata:s:s",
                f"language={lang_code}",
                f"-metadata:s:s",
                f"title=AnimeSubs_{target_language}",
                "-y",  # Overwrite output file
                output_path,
            ]

            result = subprocess.run(cmd, capture_output=True, text=True)

            if result.returncode != 0:
                raise VideoProcessingError(
                    f"FFmpeg error during embedding: {result.stderr}"
                )

            # Verify output file exists
            if not os.path.exists(output_path):
                raise VideoProcessingError("Output video file was not created")

            return True

        except Exception as e:
            raise VideoProcessingError(f"Failed to embed subtitle: {e}")

    def process_video_file(
        self,
        video_path: str,
        translator: SubtitleTranslator,
        target_language: str = "pt-BR",
        temp_dir: Optional[str] = None,
        backup_dir: Optional[str] = None,
    ) -> bool:
        """
        Process a single video file: extract, translate, and embed subtitles.

        Args:
            video_path: Path to video file
            translator: SubtitleTranslator instance
            target_language: Target language for translation
            temp_dir: Temporary directory for intermediate files
            backup_dir: Directory to save backup (if None, overwrite original)

        Returns:
            True if processing successful, False otherwise

        Raises:
            VideoProcessingError: If processing fails
        """
        video_path_obj = Path(video_path)

        if not video_path_obj.exists():
            raise VideoProcessingError(f"Video file not found: {video_path}")

        # Check if target language already exists
        if self.has_target_language(video_path, target_language):
            return False  # Skip, already has target language

        # Get subtitle tracks
        subtitle_tracks = self.get_subtitle_tracks(video_path)

        if not subtitle_tracks:
            raise VideoProcessingError(f"No subtitle tracks found in {video_path}")

        # Select best subtitle track
        best_track = self.select_best_subtitle_track(subtitle_tracks)

        if not best_track:
            raise VideoProcessingError(
                f"No suitable subtitle track found in {video_path}"
            )

        # Create temporary directory if not provided
        if temp_dir is None:
            temp_dir = tempfile.mkdtemp(prefix="animesubs_")
        else:
            os.makedirs(temp_dir, exist_ok=True)

        original_sub_path = ""
        translated_sub_path = ""

        try:
            # Extract subtitle
            original_sub_path = os.path.join(
                temp_dir, f"{video_path_obj.stem}_original.ass"
            )
            translated_sub_path = os.path.join(
                temp_dir, f"{video_path_obj.stem}_translated.ass"
            )

            # Find the actual subtitle stream index (not track index)
            stream_index = None
            video_info = self.get_video_info(video_path)

            for i, stream in enumerate(video_info.get("streams", [])):
                if (
                    stream.get("codec_type") == "subtitle"
                    and stream.get("index") == best_track["index"]
                ):
                    # Count subtitle streams before this one
                    subtitle_count = 0
                    for j in range(i):
                        if video_info["streams"][j].get("codec_type") == "subtitle":
                            subtitle_count += 1
                    stream_index = subtitle_count
                    break

            if stream_index is None:
                raise VideoProcessingError(
                    f"Could not find subtitle stream index for track {best_track['index']}"
                )

            # Extract subtitle
            self.extract_subtitle(video_path, stream_index, original_sub_path)

            # Translate subtitle
            if not translator.translate_subtitle_file(
                original_sub_path, translated_sub_path
            ):
                raise VideoProcessingError("Failed to translate subtitle")

            # Create output path (replace original file)
            ext = Path(video_path).suffix
            output_path = str(Path(video_path).with_suffix(f".tmp{ext}"))

            # Embed translated subtitle
            self.embed_subtitle(
                video_path, translated_sub_path, target_language, output_path
            )

            # Backup or overwrite
            if backup_dir:
                backup_dir_path = Path(backup_dir)
                backup_dir_path.mkdir(parents=True, exist_ok=True)
                backup_path = backup_dir_path / Path(video_path).name
                os.rename(video_path, backup_path)
            else:
                os.remove(video_path)
            os.rename(output_path, video_path)

            # Remove backup (optional - keep for safety)
            # os.remove(backup_path)

            return True

        finally:
            # Clean up temporary files
            for temp_file in [original_sub_path, translated_sub_path]:
                if os.path.exists(temp_file):
                    try:
                        os.remove(temp_file)
                    except OSError:
                        pass

    def process_directory(
        self,
        directory_path: str,
        translator: SubtitleTranslator,
        target_language: str = "pt-BR",
        video_extensions: Optional[List[str]] = None,
        recursive: bool = True,
        backup_dir: Optional[str] = None,
    ) -> Dict[str, bool]:
        """
        Process all video files in a directory.

        Args:
            directory_path: Path to directory containing video files
            translator: SubtitleTranslator instance
            target_language: Target language for translation
            video_extensions: List of video file extensions to process
            recursive: Whether to process subdirectories recursively

        Returns:
            Dictionary mapping video file paths to processing results

        Raises:
            VideoProcessingError: If directory processing fails
        """
        if video_extensions is None:
            video_extensions = [".mkv", ".mp4", ".avi", ".mov", ".wmv", ".flv", ".webm"]

        directory_path_obj = Path(directory_path)

        if not directory_path_obj.exists() or not directory_path_obj.is_dir():
            raise VideoProcessingError(f"Directory not found: {directory_path}")

        # Find video files
        video_files = []

        if recursive:
            for ext in video_extensions:
                video_files.extend(directory_path_obj.rglob(f"*{ext}"))
                video_files.extend(directory_path_obj.rglob(f"*{ext.upper()}"))
        else:
            for ext in video_extensions:
                video_files.extend(directory_path_obj.glob(f"*{ext}"))
                video_files.extend(directory_path_obj.glob(f"*{ext.upper()}"))

        # Salva argumentos do translator para criar novos por thread
        def worker(video_file):
            local_translator = SubtitleTranslator(
                api_key=translator.api_key,
                target_language=translator.target_language,
                batch_size=translator.batch_size,
                concurrence=translator.concurrence,
            )
            return self.process_video_file(
                str(video_file), local_translator, target_language, temp_dir, backup_dir
            )

        results = {}
        with tempfile.TemporaryDirectory(prefix="animesubs_batch_") as temp_dir:
            with ThreadPoolExecutor(max_workers=self.concurrence) as executor:
                future_to_video = {
                    executor.submit(worker, video_file): str(video_file)
                    for video_file in video_files
                }
                for future in as_completed(future_to_video):
                    video_file = future_to_video[future]
                    try:
                        result = future.result()
                        results[video_file] = result
                        if result:
                            print(f"✓ Successfully processed: {video_file}")
                        else:
                            print(
                                f"- Skipped (already has {target_language}): {video_file}"
                            )
                    except Exception as e:
                        print(f"✗ Failed to process {video_file}: {e}")
                        results[video_file] = False
        return results
