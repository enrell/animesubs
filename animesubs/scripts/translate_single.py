#!/usr/bin/env python3
"""
Single file subtitle translation script using AnimeSubs library.

This script provides a simple interface for translating individual subtitle files
using the AnimeSubs library with Gemini AI integration.
"""

import argparse
import os
import sys
from pathlib import Path

from dotenv import load_dotenv

from animesubs import SubtitleTranslator, TranslationError


def main():
    """Main function for single file translation."""
    parser = argparse.ArgumentParser(
        description="Translate subtitle files using AnimeSubs library"
    )
    parser.add_argument("input", help="Input subtitle file (.ass)")
    parser.add_argument("-o", "--output", help="Output file (optional)")
    parser.add_argument(
        "-l", "--language", default="pt-BR", help="Target language (default: pt-BR)"
    )
    parser.add_argument(
        "-b", "--batch-size", type=int, default=50, help="Batch size (default: 50)"
    )

    args = parser.parse_args()

    # Load environment variables
    load_dotenv()

    api_key = os.getenv("GEMINI_API_KEY")
    if not api_key:
        print("Error: GEMINI_API_KEY not found in .env file")
        print("Create a .env file based on .env.example")
        return 1

    # Validate input file
    input_path = Path(args.input)
    if not input_path.exists():
        print(f"Error: Input file not found: {args.input}")
        return 1

    # Define output file
    if args.output:
        output_path = args.output
    else:
        output_path = input_path.with_suffix(f".{args.language}{input_path.suffix}")

    try:
        # Initialize translator
        translator = SubtitleTranslator(
            api_key=api_key, target_language=args.language, batch_size=args.batch_size
        )

        # Execute translation
        print(f"Translating {args.input} to {args.language}...")
        success = translator.translate_subtitle_file(str(input_path), str(output_path))

        if success:
            print(f"Translation completed successfully: {output_path}")
            return 0
        else:
            print("Translation failed")
            return 1

    except TranslationError as e:
        print(f"Translation error: {e}")
        return 1
    except Exception as e:
        print(f"Unexpected error: {e}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
