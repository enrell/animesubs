#!/usr/bin/env python3
"""
Test script to process a single video file: extract subtitle, translate, and save.
"""

import argparse
import os
import sys
import tempfile
from pathlib import Path

from dotenv import load_dotenv

from animesubs import (
    SubtitleTranslator,
    VideoProcessor,
    VideoProcessingError,
    TranslationError,
)


def main():
    parser = argparse.ArgumentParser(
        description="Extract and translate subtitle from a single video file."
    )
    parser.add_argument("input", help="Input video file or directory")
    parser.add_argument("-o", "--output", help="Output subtitle file path (optional)")
    parser.add_argument(
        "-l", "--language", default="pt-BR", help="Target language (default: pt-BR)"
    )
    parser.add_argument(
        "-s",
        "--source-language",
        default="Japanese",
        help="Source language (default: Japanese)",
    )
    parser.add_argument(
        "--provider",
        default="gemini",
        choices=["gemini", "openai", "ollama", "lmstudio"],
        help="Translation provider (default: gemini)",
    )
    parser.add_argument("--model", help="Model name to use")
    parser.add_argument("--api-key", help="API key for the provider")
    parser.add_argument(
        "--base-url", help="Base URL for the provider (e.g. for OpenAI compatible APIs)"
    )
    parser.add_argument(
        "-b",
        "--batch-size",
        type=int,
        help="Batch size for translation (default: 300 for cloud, 50 for local)",
    )

    args = parser.parse_args()

    # Load environment variables
    load_dotenv()

    # Determine API key based on provider
    api_key = args.api_key
    if not api_key:
        if args.provider == "gemini":
            api_key = os.getenv("GEMINI_API_KEY")
        elif args.provider == "openai":
            api_key = os.getenv("OPENAI_API_KEY")

    # Gemini requires API key, OpenAI usually does too unless local
    if args.provider in ["gemini", "openai"] and not api_key and not args.base_url:
        # If base_url is provided for OpenAI, maybe key is not needed (e.g. local server)
        # But generally we warn.
        if args.provider == "gemini":
            print("Error: GEMINI_API_KEY not found in .env file or --api-key argument")
            return 1
        elif args.provider == "openai" and not args.base_url:
            print("Error: OPENAI_API_KEY not found in .env file or --api-key argument")
            return 1

    input_path = Path(args.input)

    # Handle directory input by picking the first video file
    if input_path.is_dir():
        print(f"Input is a directory. Searching for video files in {input_path}...")
        video_extensions = {".mkv", ".mp4", ".avi"}
        video_files = [
            p
            for p in input_path.iterdir()
            if p.is_file() and p.suffix.lower() in video_extensions
        ]
        if not video_files:
            print("No video files found in directory.")
            return 1
        input_path = video_files[0]
        print(f"Selected video file: {input_path}")
    elif not input_path.exists():
        print(f"Error: Input file not found: {args.input}")
        return 1

    # Determine output path
    if args.output:
        output_path = Path(args.output)
    else:
        # Default to ./filename.lang.ass
        output_path = Path.cwd() / f"{input_path.stem}.{args.language}.ass"

    print(f"Processing: {input_path}")
    print(f"Target Output: {output_path}")
    print(f"Provider: {args.provider}")

    try:
        processor = VideoProcessor()

        # Prepare provider kwargs
        provider_kwargs = {}
        if args.model:
            provider_kwargs["model"] = args.model
        if args.base_url:
            provider_kwargs["base_url"] = args.base_url
        if args.provider == "ollama" and args.base_url:
            # Ollama provider uses 'host'
            provider_kwargs["host"] = args.base_url

        # Determine batch size
        batch_size = args.batch_size
        if not batch_size:
            batch_size = 200

        translator = SubtitleTranslator(
            api_key=api_key,
            source_language=args.source_language,
            target_language=args.language,
            provider=args.provider,
            batch_size=batch_size,
            **provider_kwargs,
        )

        # 1. Identify subtitle track
        print("Analyzing video tracks...")
        tracks = processor.get_subtitle_tracks(str(input_path))
        best_track = processor.select_best_subtitle_track(tracks)

        if not best_track:
            print("Error: No suitable subtitle track found.")
            return 1

        print(
            f"Selected track: {best_track['index']} ({best_track.get('language', 'unknown')}) - {best_track.get('title', 'No Title')}"
        )

        # 2. Extract subtitle
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_sub_path = Path(temp_dir) / "extracted.ass"
            print("Extracting subtitle...")

            # Check if we can copy or need to convert
            codec = best_track.get("codec_name", "").lower()
            copy_stream = codec in ["ass", "ssa"]

            processor.ffmpeg.extract_subtitle(
                str(input_path),
                int(best_track["index"]),
                str(temp_sub_path),
                copy=copy_stream,
            )

            # 3. Translate
            print("Translating subtitle...")
            translator.translate_subtitle_file(str(temp_sub_path), str(output_path))

        print(f"Done! Translated subtitle saved to: {output_path}")
        return 0

    except (VideoProcessingError, TranslationError) as e:
        print(f"Error: {e}")
        return 1
    except Exception as e:
        print(f"Unexpected error: {e}")
        import traceback

        traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(main())
