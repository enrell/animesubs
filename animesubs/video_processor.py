import json
import os
import tempfile
from concurrent.futures import ThreadPoolExecutor, as_completed
from pathlib import Path
from typing import Any, Dict, List, Optional

from .constants import LANGUAGE_CODES, PREFERRED_LANGUAGES
from .exceptions import VideoProcessingError
from .ffmpeg import FFmpegWrapper
from .translator import SubtitleTranslator


class VideoProcessor:
    def __init__(
        self,
        ffmpeg_path: str = "ffmpeg",
        ffprobe_path: str = "ffprobe",
        concurrence: int = 1,
    ):
        self.ffmpeg = FFmpegWrapper(ffmpeg_path, ffprobe_path)
        self.concurrence = concurrence

    def get_video_info(self, video_path: str) -> Dict[str, Any]:
        try:
            json_output = self.ffmpeg.get_video_info_json(video_path)
            return json.loads(json_output)
        except json.JSONDecodeError as e:
            raise VideoProcessingError(f"Failed to parse video info: {e}")

    def get_subtitle_tracks(self, video_path: str) -> List[Dict[str, Any]]:
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
        subtitle_tracks = self.get_subtitle_tracks(video_path)
        target_code = LANGUAGE_CODES.get(target_language, target_language)
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
        if not subtitle_tracks:
            return None

        def get_language_priority(track):
            lang = track["language"].lower()
            for i, preferred in enumerate(PREFERRED_LANGUAGES):
                if lang.startswith(preferred.lower()):
                    return i
            return len(PREFERRED_LANGUAGES)

        dialogue_tracks = []
        for track in subtitle_tracks:
            codec = track.get("codec_name", "").lower()
            if codec not in ["ass", "ssa", "subrip", "webvtt", "mov_text", "text"]:
                continue

            title = track.get("title", "").lower()
            if any(
                keyword in title
                for keyword in ["sign", "song", "opening", "ending", "credit"]
            ):
                continue
            dialogue_tracks.append(track)

        if not dialogue_tracks:
            dialogue_tracks = subtitle_tracks

        dialogue_tracks.sort(
            key=lambda x: (get_language_priority(x), -int(x.get("frame_count", "0")))
        )
        return dialogue_tracks[0] if dialogue_tracks else None

    def process_video_file(
        self,
        video_path: str,
        translator: SubtitleTranslator,
        target_language: str = "pt-BR",
        temp_dir: Optional[str] = None,
        backup_dir: Optional[str] = None,
    ) -> bool:
        video_path_obj = Path(video_path)
        if not video_path_obj.exists():
            raise VideoProcessingError(f"Video file not found: {video_path}")

        if self.has_target_language(video_path, target_language):
            return False

        subtitle_tracks = self.get_subtitle_tracks(video_path)
        if not subtitle_tracks:
            raise VideoProcessingError(f"No subtitle tracks found in {video_path}")

        best_track = self.select_best_subtitle_track(subtitle_tracks)
        if not best_track:
            raise VideoProcessingError(
                f"No suitable subtitle track found in {video_path}"
            )

        if temp_dir is None:
            temp_dir = tempfile.mkdtemp(prefix="animesubs_")
        else:
            os.makedirs(temp_dir, exist_ok=True)

        original_sub_path = ""
        translated_sub_path = ""

        try:
            original_sub_path = os.path.join(
                temp_dir, f"{video_path_obj.stem}_original.ass"
            )
            translated_sub_path = os.path.join(
                temp_dir, f"{video_path_obj.stem}_translated.ass"
            )

            stream_index = best_track["index"]
            codec = best_track.get("codec_name", "").lower()
            should_copy = codec in ["ass", "ssa"]
            self.ffmpeg.extract_subtitle(
                video_path, stream_index, original_sub_path, copy=should_copy
            )

            if not translator.translate_subtitle_file(
                original_sub_path, translated_sub_path
            ):
                raise VideoProcessingError("Failed to translate subtitle")

            ext = Path(video_path).suffix
            output_path = str(Path(video_path).with_suffix(f".tmp{ext}"))

            lang_code = LANGUAGE_CODES.get(target_language, target_language)
            new_sub_index = len(subtitle_tracks)
            metadata_args = [
                f"-metadata:s:s:{new_sub_index}",
                f"language={lang_code}",
                f"-metadata:s:s:{new_sub_index}",
                f"title=AnimeSubs_{target_language}",
            ]

            self.ffmpeg.embed_subtitle(
                video_path, translated_sub_path, output_path, metadata_args
            )

            if backup_dir:
                backup_dir_path = Path(backup_dir)
                backup_dir_path.mkdir(parents=True, exist_ok=True)
                backup_path = backup_dir_path / Path(video_path).name
                os.rename(video_path, backup_path)
            else:
                os.remove(video_path)
            os.rename(output_path, video_path)

            return True

        finally:
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
        if video_extensions is None:
            video_extensions = [".mkv", ".mp4", ".avi", ".mov", ".wmv", ".flv", ".webm"]

        directory_path_obj = Path(directory_path)
        if not directory_path_obj.exists() or not directory_path_obj.is_dir():
            raise VideoProcessingError(f"Directory not found: {directory_path}")

        video_files = []
        if recursive:
            for ext in video_extensions:
                video_files.extend(directory_path_obj.rglob(f"*{ext}"))
                video_files.extend(directory_path_obj.rglob(f"*{ext.upper()}"))
        else:
            for ext in video_extensions:
                video_files.extend(directory_path_obj.glob(f"*{ext}"))
                video_files.extend(directory_path_obj.glob(f"*{ext.upper()}"))

        shared_cache = {}

        def worker(video_file):
            local_translator = SubtitleTranslator(
                api_key=translator.api_key,
                target_language=translator.target_language,
                batch_size=translator.batch_size,
                concurrence=translator.concurrence,
                translations_cache=shared_cache,
                provider=translator.provider_name,
                **translator.provider_kwargs,
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
