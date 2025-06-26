"""
Tests for VideoProcessor class.
"""

import json
import subprocess
from unittest.mock import Mock, patch

import pytest

from animesubs import SubtitleExtractionError, VideoProcessingError, VideoProcessor


class TestVideoProcessor:
    """Test cases for VideoProcessor class."""

    def test_init_with_default_paths(self):
        """Test VideoProcessor initialization with default FFmpeg paths."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            processor = VideoProcessor()
            assert processor.ffmpeg_path == "ffmpeg"
            assert processor.ffprobe_path == "ffprobe"

    def test_init_with_custom_paths(self):
        """Test VideoProcessor initialization with custom FFmpeg paths."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            processor = VideoProcessor(
                ffmpeg_path="/usr/bin/ffmpeg", ffprobe_path="/usr/bin/ffprobe"
            )
            assert processor.ffmpeg_path == "/usr/bin/ffmpeg"
            assert processor.ffprobe_path == "/usr/bin/ffprobe"

    def test_verify_ffmpeg_success(self):
        """Test successful FFmpeg verification."""
        with patch("subprocess.run") as mock_run:
            mock_run.return_value = Mock(returncode=0)

            # Should not raise exception
            processor = VideoProcessor()
            assert processor.ffmpeg_path == "ffmpeg"

    def test_verify_ffmpeg_failure(self):
        """Test FFmpeg verification failure."""
        with patch("subprocess.run", side_effect=FileNotFoundError("FFmpeg not found")):
            with pytest.raises(VideoProcessingError, match="FFmpeg not available"):
                VideoProcessor()

    def test_get_video_info_success(self, mock_video_info):
        """Test successful video info retrieval."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch("subprocess.run") as mock_run:
                mock_run.return_value = Mock(
                    returncode=0, stdout=json.dumps(mock_video_info)
                )

                processor = VideoProcessor()
                info = processor.get_video_info("/path/to/video.mkv")

                # Fix: Ensure info is not None before subscripting
                assert info is not None
                assert info["streams"][0]["codec_name"] == "h264"
                assert info["streams"][1]["codec_type"] == "audio"
                assert info["streams"][2]["tags"]["language"] == "eng"

    def test_get_video_info_failure(self):
        """Test video info retrieval failure."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch("subprocess.run") as mock_run:
                mock_run.side_effect = subprocess.CalledProcessError(1, "ffprobe")

                processor = VideoProcessor()

                with pytest.raises(
                    VideoProcessingError, match="Failed to get video info"
                ):
                    processor.get_video_info("/path/to/video.mkv")

    def test_get_subtitle_tracks(self, mock_video_info):
        """Test subtitle track extraction."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch.object(
                VideoProcessor, "get_video_info", return_value=mock_video_info
            ):
                processor = VideoProcessor()
                tracks = processor.get_subtitle_tracks("/path/to/video.mkv")

                assert len(tracks) == 2  # Two subtitle streams in mock data
                assert tracks[0]["language"] == "eng"
                assert tracks[1]["language"] == "jpn"
                assert tracks[0]["frame_count"] == "356"
                assert tracks[1]["frame_count"] == "400"

    def test_has_target_language_true(self, mock_video_info):
        """Test target language detection when language exists."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch.object(
                VideoProcessor, "get_video_info", return_value=mock_video_info
            ):
                processor = VideoProcessor()

                # Should find English
                assert processor.has_target_language("/path/to/video.mkv", "en") is True
                assert (
                    processor.has_target_language("/path/to/video.mkv", "eng") is True
                )

    def test_has_target_language_false(self, mock_video_info):
        """Test target language detection when language doesn't exist."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch.object(
                VideoProcessor, "get_video_info", return_value=mock_video_info
            ):
                processor = VideoProcessor()

                # Should not find Portuguese
                assert (
                    processor.has_target_language("/path/to/video.mkv", "pt-BR")
                    is False
                )
                assert (
                    processor.has_target_language("/path/to/video.mkv", "por") is False
                )

    def test_select_best_subtitle_track(self):
        """Test best subtitle track selection."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            processor = VideoProcessor()

            tracks = [
                {
                    "index": 2,
                    "language": "jpn",
                    "title": "Japanese",
                    "frame_count": "400",
                },
                {
                    "index": 3,
                    "language": "eng",
                    "title": "English",
                    "frame_count": "356",
                },
                {
                    "index": 4,
                    "language": "eng",
                    "title": "English Signs",
                    "frame_count": "50",
                },
            ]

            best_track = processor.select_best_subtitle_track(tracks)

            # Should prefer English over Japanese, and full subtitles over signs
            assert best_track is not None
            assert best_track["language"] == "eng"
            assert "Signs" not in best_track["title"]
            assert best_track["frame_count"] == "356"

    def test_select_best_subtitle_track_empty(self):
        """Test best subtitle track selection with empty list."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            processor = VideoProcessor()

            result = processor.select_best_subtitle_track([])
            assert result is None

    def test_extract_subtitle_success(self, temp_dir, mock_ffmpeg_success):
        """Test successful subtitle extraction."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch("subprocess.run", return_value=mock_ffmpeg_success):
                with patch("os.path.exists", return_value=True):
                    with patch("os.path.getsize", return_value=1024):
                        processor = VideoProcessor()

                        output_path = temp_dir / "extracted.ass"
                        result = processor.extract_subtitle(
                            "/path/to/video.mkv", 0, str(output_path)
                        )

                        assert result is True

    def test_extract_subtitle_ffmpeg_error(self):
        """Test subtitle extraction with FFmpeg error."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            mock_result = Mock()
            mock_result.returncode = 1
            mock_result.stderr = "FFmpeg error"

            with patch("subprocess.run", return_value=mock_result):
                processor = VideoProcessor()

                with pytest.raises(SubtitleExtractionError, match="FFmpeg error"):
                    processor.extract_subtitle(
                        "/path/to/video.mkv", 0, "/output/path.ass"
                    )

    def test_extract_subtitle_empty_output(self, mock_ffmpeg_success):
        """Test subtitle extraction with empty output file."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch("subprocess.run", return_value=mock_ffmpeg_success):
                with patch("os.path.exists", return_value=True):
                    with patch("os.path.getsize", return_value=0):  # Empty file
                        processor = VideoProcessor()

                        with pytest.raises(SubtitleExtractionError, match="empty"):
                            processor.extract_subtitle(
                                "/path/to/video.mkv", 0, "/output/path.ass"
                            )

    def test_embed_subtitle_success(self, mock_ffmpeg_success):
        """Test successful subtitle embedding."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch("subprocess.run", return_value=mock_ffmpeg_success):
                with patch("os.path.exists", return_value=True):
                    processor = VideoProcessor()

                    result = processor.embed_subtitle(
                        "/path/to/video.mkv",
                        "/path/to/subtitle.ass",
                        "pt-BR",
                        "/path/to/output.mkv",
                    )

                    assert result is True

    def test_embed_subtitle_ffmpeg_error(self):
        """Test subtitle embedding with FFmpeg error."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            mock_result = Mock()
            mock_result.returncode = 1
            mock_result.stderr = "FFmpeg embedding error"

            with patch("subprocess.run", return_value=mock_result):
                processor = VideoProcessor()

                with pytest.raises(
                    VideoProcessingError, match="FFmpeg error during embedding"
                ):
                    processor.embed_subtitle(
                        "/path/to/video.mkv",
                        "/path/to/subtitle.ass",
                        "pt-BR",
                        "/path/to/output.mkv",
                    )

    def test_process_video_file_skip_existing_language(self, mock_video_info):
        """Test video processing when target language already exists."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch.object(VideoProcessor, "has_target_language", return_value=True):
                with patch("pathlib.Path.exists", return_value=True):
                    processor = VideoProcessor()
                    mock_translator = Mock()

                    result = processor.process_video_file(
                        "/path/to/video.mkv", mock_translator, "eng"
                    )

                # Should skip and return False
                assert result is False

    def test_process_video_file_no_subtitles(self):
        """Test video processing when no subtitle tracks exist."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            with patch.object(
                VideoProcessor, "has_target_language", return_value=False
            ):
                with patch.object(
                    VideoProcessor, "get_subtitle_tracks", return_value=[]
                ):
                    with patch("pathlib.Path.exists", return_value=True):
                        processor = VideoProcessor()
                        mock_translator = Mock()

                        with pytest.raises(
                            VideoProcessingError, match="No subtitle tracks found"
                        ):
                            processor.process_video_file(
                                "/path/to/video.mkv", mock_translator
                            )

    def test_process_directory_success(self, temp_dir):
        """Test successful directory processing."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            # Create mock video files
            video_files = [temp_dir / "video1.mkv", temp_dir / "video2.mp4"]
            for video_file in video_files:
                video_file.touch()

            with patch.object(VideoProcessor, "process_video_file", return_value=True):
                processor = VideoProcessor()
                mock_translator = Mock()

                results = processor.process_directory(
                    str(temp_dir), mock_translator, recursive=False
                )

                assert len(results) == 2
                assert all(result is True for result in results.values())

    def test_process_directory_not_found(self):
        """Test directory processing with non-existent directory."""
        with patch.object(VideoProcessor, "_verify_ffmpeg"):
            processor = VideoProcessor()
            mock_translator = Mock()

            with pytest.raises(VideoProcessingError, match="Directory not found"):
                processor.process_directory("/non/existent/path", mock_translator)
