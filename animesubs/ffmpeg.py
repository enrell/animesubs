import os
import subprocess
from typing import List, Optional

from .exceptions import FFmpegError


class FFmpegWrapper:
    def __init__(self, ffmpeg_path: str = "ffmpeg", ffprobe_path: str = "ffprobe"):
        self.ffmpeg_path = ffmpeg_path
        self.ffprobe_path = ffprobe_path
        self._verify_ffmpeg()

    def _verify_ffmpeg(self) -> None:
        try:
            subprocess.run(
                [self.ffmpeg_path, "-version"], capture_output=True, check=True
            )
            subprocess.run(
                [self.ffprobe_path, "-version"], capture_output=True, check=True
            )
        except (subprocess.CalledProcessError, FileNotFoundError) as e:
            raise FFmpegError(f"FFmpeg not available: {e}")

    def run_command(
        self, cmd: List[str], check: bool = True
    ) -> subprocess.CompletedProcess:
        try:
            return subprocess.run(cmd, capture_output=True, text=True, check=check)
        except subprocess.CalledProcessError as e:
            raise FFmpegError(f"FFmpeg command failed: {e.stderr}") from e

    def get_video_info_json(self, video_path: str) -> str:
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
        result = self.run_command(cmd)
        return result.stdout

    def extract_subtitle(
        self, video_path: str, stream_index: int, output_path: str, copy: bool = False
    ) -> None:
        cmd = [
            self.ffmpeg_path,
            "-i",
            video_path,
            "-map",
            f"0:{stream_index}",
        ]
        if copy:
            cmd.extend(["-c", "copy"])

        cmd.extend(["-y", output_path])
        self.run_command(cmd)

    def embed_subtitle(
        self,
        video_path: str,
        subtitle_path: str,
        output_path: str,
        metadata_args: List[str],
    ) -> None:
        cmd = [
            self.ffmpeg_path,
            "-i",
            video_path,
            "-i",
            subtitle_path,
            "-c",
            "copy",
            "-map",
            "0",
            "-map",
            "1",
        ]
        cmd.extend(metadata_args)
        cmd.extend(["-y", output_path])
        self.run_command(cmd)
