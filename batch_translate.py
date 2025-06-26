#!/usr/bin/env python3
"""
AnimeSubs - Batch subtitle translation script for video folders.

This script processes all video files in a directory, extracts subtitles,
translates them using AI, and embeds the translated subtitles back into
the video files.

Features:
- Process entire directories of video files
- Automatically detect and extract best subtitle tracks
- Skip videos that already have target language subtitles
- Support for multiple video formats (MKV, MP4, AVI, etc.)
- Backup original files for safety
- Detailed progress reporting
"""

import argparse
import os
import sys
from pathlib import Path

from dotenv import load_dotenv

from animesubs import (
    SubtitleTranslator,
    TranslationError,
    VideoProcessingError,
    VideoProcessor,
)


def main():
    """Main function for batch video processing."""
    parser = argparse.ArgumentParser(
        description="Batch translate subtitles in video files using AnimeSubs",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
Examples:
  # Process all MKV files in current directory
  python batch_translate.py /path/to/videos
  
  # Process with specific target language
  python batch_translate.py /path/to/videos -l es
  
  # Process only MKV files, non-recursive
  python batch_translate.py /path/to/videos --extensions .mkv --no-recursive
  
  # Use custom batch size for translation
  python batch_translate.py /path/to/videos -b 100
        """,
    )

    parser.add_argument("directory", help="Directory containing video files to process")

    parser.add_argument(
        "-l",
        "--language",
        default="pt-BR",
        help="Target language for translation (default: pt-BR)",
    )

    parser.add_argument(
        "-b",
        "--batch-size",
        type=int,
        default=1000,
        help="Batch size for translation API calls (default: 1000)",
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
        "-v", "--verbose", action="store_true", help="Enable verbose output"
    )

    parser.add_argument(
        "-B",
        "--backup",
        action="store_true",
        help="Save backups of original files in a 'backup' folder instead of overwriting.",
    )

    parser.add_argument(
        "-c",
        "--concurrence",
        type=int,
        default=1,
        help="Number of concurrent workers for extraction and LLM requests (default: 1)",
    )

    args = parser.parse_args()

    # Load environment variables
    load_dotenv()

    # Get API key
    api_key = os.getenv("GEMINI_API_KEY")
    if not api_key:
        print("Error: GEMINI_API_KEY not found in environment")
        print("Please set your Gemini API key in .env file or environment variable")
        return 1

    # Validate directory
    directory = Path(args.directory)
    if not directory.exists():
        print(f"Error: Directory not found: {args.directory}")
        return 1

    if not directory.is_dir():
        print(f"Error: Path is not a directory: {args.directory}")
        return 1

    try:
        # Initialize components
        print("Initializing AnimeSubs components...")

        translator = SubtitleTranslator(
            api_key=api_key,
            target_language=args.language,
            batch_size=args.batch_size,
            concurrence=args.concurrence,
        )

        video_processor = VideoProcessor(
            ffmpeg_path=args.ffmpeg_path,
            ffprobe_path=args.ffprobe_path,
            concurrence=args.concurrence,
        )

        print(f"Target language: {args.language}")
        print(f"Batch size: {args.batch_size}")
        print(f"Video extensions: {', '.join(args.extensions)}")
        print(f"Recursive: {not args.no_recursive}")
        print(f"Directory: {args.directory}")
        print()

        if args.dry_run:
            print("DRY RUN MODE - No files will be modified")
            print()

            # Find video files
            directory_path = Path(args.directory)
            video_files = []

            if not args.no_recursive:
                for ext in args.extensions:
                    video_files.extend(directory_path.rglob(f"*{ext}"))
                    video_files.extend(directory_path.rglob(f"*{ext.upper()}"))
            else:
                for ext in args.extensions:
                    video_files.extend(directory_path.glob(f"*{ext}"))
                    video_files.extend(directory_path.glob(f"*{ext.upper()}"))

            print(f"Found {len(video_files)} video files:")
            for video_file in sorted(video_files):
                # Check if already has target language
                has_target = video_processor.has_target_language(
                    str(video_file), args.language
                )
                status = "SKIP (has target language)" if has_target else "PROCESS"
                print(f"  {status}: {video_file}")

            return 0

        # Process directory
        print("Starting batch processing...")
        backup_dir = None
        if args.backup:
            backup_dir = str(Path(args.directory) / "backup")
        results = video_processor.process_directory(
            str(directory),
            translator,
            target_language=args.language,
            video_extensions=args.extensions,
            recursive=not args.no_recursive,
            backup_dir=backup_dir,
        )

        # Print summary
        print("\n" + "=" * 60)
        print("PROCESSING SUMMARY")
        print("=" * 60)

        total_files = len(results)
        successful = sum(1 for success in results.values() if success)
        skipped = sum(1 for success in results.values() if success is False)
        failed = total_files - successful - skipped

        print(f"Total files: {total_files}")
        print(f"Successfully processed: {successful}")
        print(f"Skipped (already has {args.language}): {skipped}")
        print(f"Failed: {failed}")

        if args.verbose:
            print("\nDetailed results:")
            for file_path, result in results.items():
                if result is True:
                    status = "SUCCESS"
                elif result is False:
                    status = "SKIPPED"
                else:
                    status = "FAILED"
                print(f"  {status}: {file_path}")

        print("\nNOTE: Original files have been backed up with .backup extension")
        print("You can remove backup files once you've verified the results")

        return 0 if failed == 0 else 1

    except TranslationError as e:
        print(f"Translation error: {e}")
        return 1

    except VideoProcessingError as e:
        print(f"Video processing error: {e}")
        return 1

    except KeyboardInterrupt:
        print("\nOperation cancelled by user")
        return 1

    except Exception as e:
        print(f"Unexpected error: {e}")
        if args.verbose:
            import traceback

            traceback.print_exc()
        return 1


if __name__ == "__main__":
    sys.exit(main())
