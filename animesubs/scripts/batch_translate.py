import argparse
import os
import sys
from pathlib import Path

from dotenv import load_dotenv

from animesubs import (
    SubtitleTranslator,
    VideoProcessor,
)


def main():
    parser = argparse.ArgumentParser(
        description="Batch translate subtitles in video files using AnimeSubs",
        formatter_class=argparse.RawDescriptionHelpFormatter,
    )

    parser.add_argument("directory", help="Directory containing video files to process")

    parser.add_argument(
        "-l",
        "--language",
        default="pt-BR",
        help="Target language for translation (default: pt-BR)",
    )

    parser.add_argument(
        "-s",
        "--source-language",
        default="Japanese",
        help="Source language for translation (default: Japanese)",
    )

    parser.add_argument(
        "-b",
        "--batch-size",
        type=int,
        default=None,
        help="Batch size for translation API calls (default: 300 for cloud, 50 for local)",
    )

    parser.add_argument(
        "--extensions",
        nargs="+",
        default=[".mkv", ".mp4", ".avi", ".mov", ".wmv", ".flv", ".webm"],
        help="Video file extensions to process (default: .mkv .mp4 .avi .mov .wmv .flv .webm)",
    )

    parser.add_argument(
        "--no-recursive",
        action="store_true",
        help="Don't process subdirectories recursively",
    )

    parser.add_argument(
        "--ffmpeg-path",
        default="ffmpeg",
        help="Path to FFmpeg executable (default: ffmpeg)",
    )

    parser.add_argument(
        "--ffprobe-path",
        default="ffprobe",
        help="Path to FFprobe executable (default: ffprobe)",
    )

    parser.add_argument(
        "--dry-run",
        action="store_true",
        help="Show what would be processed without actually doing it",
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

    args = parser.parse_args()

    # Set default batch size if not specified
    if args.batch_size is None:
        args.batch_size = 300

    load_dotenv()

    # Determine API key based on provider
    api_key = args.api_key
    if not api_key:
        if args.provider == "gemini":
            api_key = os.getenv("GEMINI_API_KEY")
        elif args.provider == "openai":
            api_key = os.getenv("OPENAI_API_KEY")
        elif args.provider == "ollama":
            api_key = os.getenv("OLLAMA_API_KEY")
        elif args.provider == "lmstudio":
            api_key = os.getenv("LMSTUDIO_API_KEY")

    # Gemini requires API key, OpenAI usually does too unless local
    if args.provider in ["gemini", "openai"] and not api_key and not args.base_url:
        if args.provider == "gemini":
            print("Error: GEMINI_API_KEY not found in .env file or --api-key argument")
            return 1
        elif args.provider == "openai" and not args.base_url:
            print("Error: OPENAI_API_KEY not found in .env file or --api-key argument")
            return 1

    try:
        # Prepare provider kwargs
        provider_kwargs = {}
        if args.model:
            provider_kwargs["model"] = args.model
        if args.base_url:
            provider_kwargs["base_url"] = args.base_url
        if args.provider == "ollama" and args.base_url:
            # Ollama provider uses 'host'
            provider_kwargs["host"] = args.base_url

        translator = SubtitleTranslator(
            api_key=api_key,
            source_language=args.source_language,
            target_language=args.language,
            batch_size=args.batch_size,
            provider=args.provider,
            **provider_kwargs,
        )

        processor = VideoProcessor(
            ffmpeg_path=args.ffmpeg_path, ffprobe_path=args.ffprobe_path
        )

        print(f"Processing directory: {args.directory}")
        print(f"Target language: {args.language}")

        if args.dry_run:
            print("Dry run mode - no changes will be made")
            return 0

        results = processor.process_directory(
            args.directory,
            translator,
            target_language=args.language,
            video_extensions=args.extensions,
            recursive=not args.no_recursive,
        )

        success_count = sum(1 for r in results.values() if r)
        print(f"\nProcessed {len(results)} files: {success_count} successful")

        return 0

    except Exception as e:
        print(f"Error: {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
