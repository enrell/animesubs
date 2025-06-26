#!/usr/bin/env python3
"""
Advanced subtitle translator module for AnimeSubs library.

This module provides the SubtitleTranslator class for translating subtitle files
using AI with advanced optimizations including batch processing, duplicate removal,
and intelligent text filtering.
"""

import hashlib
import re
from collections import defaultdict
from typing import Dict, List, Set

import pysubs2
from google import genai
from google.genai import types

from .exceptions import TranslationError


class SubtitleTranslator:
    """
    Advanced subtitle translator with AI integration and optimizations.

    Features:
    - Remove duplicates to optimize API usage
    - Remove metadata, keeping only dialogues
    - Process in batches
    - Rebuild translated subtitles with mapped duplicates
    - Support for multiple languages
    - Intelligent text filtering
    """

    def __init__(
        self,
        api_key: str,
        target_language: str = "pt-BR",
        batch_size: int = 300,
        concurrence: int = 1,
    ):
        """
        Initialize the subtitle translator.

        Args:
            api_key: Google Gemini API key
            target_language: Target language for translation
            batch_size: Batch size for processing
            concurrence: Number of concurrent workers for LLM requests

        Raises:
            TranslationError: If API key is invalid or client initialization fails
        """
        self.target_language = target_language
        self.batch_size = batch_size
        self.concurrence = concurrence
        self.api_key = api_key

        try:
            # Configure Gemini (python-genai)
            self.client = genai.Client(api_key=api_key)
            self.model = "gemini-2.5-flash-lite-preview-06-17"
        except Exception as e:
            raise TranslationError(f"Failed to initialize Gemini client: {e}")

        # Dictionaries for optimization
        self.unique_texts: Dict[str, str] = {}  # hash -> original text
        self.translations_cache: Dict[str, str] = {}  # hash -> translation
        self.text_mappings: Dict[int, str] = {}  # line index -> hash

    def is_technical_command(self, text: str) -> bool:
        """
        Detect if text is a technical ASS command or similar.

        Args:
            text: Text to be verified

        Returns:
            True if it's a technical command, False otherwise
        """
        text = text.strip()

        # Specific ASS drawing commands like "m 0 0 l 150 0 150 150 0 150"
        if re.match(r"^m\s+[\d\s\-\.]+l\s+[\d\s\-\.]+$", text, re.IGNORECASE):
            return True

        # ASS drawing commands (m, l, b, s, p with numbers)
        if re.match(r"^[mlbsp](\s+[\d\s\-\.]+)+$", text, re.IGNORECASE):
            return True

        # Pure numeric coordinates (only numbers, spaces, dots and hyphens)
        if re.match(r"^[\d\s\-\.]+$", text) and len(text.split()) > 2:
            return True

        # Technical ASS tags
        if re.match(r"^\\[a-zA-Z]+(\([^)]*\))?$", text):
            return True

        return False

    def is_meaningful_text(self, text: str) -> bool:
        """
        Check if text is meaningful enough for translation.

        Args:
            text: Text to be verified

        Returns:
            True if text is meaningful, False otherwise
        """
        text = text.strip()

        # Text too short (less than 4 characters to be more restrictive)
        if len(text) < 4:
            return False

        # Only numbers or symbols
        if re.match(r"^[^\w]*$", text, re.UNICODE):
            return False

        # Only one or two repeated letters
        if re.match(r"^([a-zA-Z])\1*$", text):
            return False

        # Very short common interjections
        short_interjections = {
            "ah",
            "oh",
            "eh",
            "uh",
            "hm",
            "mm",
            "ng",
            "sh",
            "ha",
            "he",
            "hi",
            "ho",
            "hu",
            "huh",
            "hmm",
            "aha",
            "ooh",
            "aah",
            "err",
            "umm",
            "uhh",
            "ehh",
            "ohh",
            "whoa",
            "wow",
            "ouch",
            "oof",
        }
        if text.lower() in short_interjections:
            return False

        # Common word fragments that don't make sense alone
        fragments = {
            "ing",
            "ed",
            "er",
            "ly",
            "tion",
            "ness",
            "ment",
            "ful",
            "less",
            "able",
            "ible",
            "ous",
            "ive",
            "ant",
            "ent",
            "ist",
            "ism",
            "ade",
            "age",
            "ary",
            "ate",
            "dom",
            "ery",
            "fy",
            "ify",
            "ize",
            "ise",
            "ward",
            "wise",
            "like",
            "ship",
            "hood",
        }
        if text.lower() in fragments:
            return False

        return True

    def clean_subtitle_text(self, text: str) -> str:
        """
        Remove metadata and unnecessary formatting, keeping only dialogue.

        Args:
            text: Original subtitle text

        Returns:
            Clean text or empty string if not relevant
        """
        if not text or text.strip() == "":
            return ""

        # Remove HTML/ASS tags
        text = re.sub(r"<[^>]+>", "", text)
        text = re.sub(r"{[^}]+}", "", text)

        # Remove extra line breaks and spaces
        text = re.sub(r"\\N", " ", text)
        text = re.sub(r"\s+", " ", text).strip()

        # Check if empty after cleaning
        if not text:
            return ""

        # Filter technical commands
        if self.is_technical_command(text):
            return ""

        # Filter non-meaningful texts
        if not self.is_meaningful_text(text):
            return ""

        return text

    def generate_text_hash(self, text: str) -> str:
        """Generate unique hash for text."""
        return hashlib.md5(text.encode("utf-8")).hexdigest()

    def extract_unique_texts(
        self, subtitle_file: pysubs2.SSAFile
    ) -> Dict[str, Set[int]]:
        """
        Extract unique texts from subtitles and map their occurrences.

        Args:
            subtitle_file: Loaded subtitle file

        Returns:
            Dictionary mapping hash -> set of line indices
        """
        unique_texts_map = defaultdict(set)
        filtered_count = 0
        technical_count = 0
        short_count = 0

        for i, line in enumerate(subtitle_file):
            original_text = line.text
            clean_text = self.clean_subtitle_text(original_text)

            if not clean_text:
                filtered_count += 1
                # Detect filter type for statistics
                if original_text and self.is_technical_command(original_text.strip()):
                    technical_count += 1
                elif original_text and len(original_text.strip()) < 3:
                    short_count += 1
                continue

            text_hash = self.generate_text_hash(clean_text)
            self.unique_texts[text_hash] = clean_text
            self.text_mappings[i] = text_hash
            unique_texts_map[text_hash].add(i)

        return dict(unique_texts_map)

    def create_translation_prompt(self, texts: List[str]) -> str:
        """
        Create optimized prompt for batch translation.

        Args:
            texts: List of unique texts to translate

        Returns:
            Formatted prompt
        """
        numbered_texts = []
        for i, text in enumerate(texts, 1):
            numbered_texts.append(f"{i}: {text}")

        prompt = f"""Translate the following anime subtitles to {self.target_language}. 
Keep the same numbered format in your response.
Preserve dialogue naturalness and context.

ANIME TRANSLATION GUIDELINES:
- Keep Japanese honorifics: -san, -sama, -kun, -chan, -senpai, -kohai, -sensei, etc.
- Preserve cultural terms: onii-chan, onee-chan, otaku, kawaii, baka, etc.
- Maintain character speech patterns and personality
- Keep anime-specific expressions and exclamations
- Preserve names as they are (don't translate character names)
- Keep attack names and technique names in original form when culturally significant
- Maintain the emotional tone and style appropriate for anime dialogue

Don't add explanations, just translate:

{chr(10).join(numbered_texts)}

Response:"""

        return prompt

    def parse_translation_response(
        self, response: str, original_texts: List[str]
    ) -> Dict[str, str]:
        """
        Parse translation response and map to original texts.

        Args:
            response: AI model response
            original_texts: List of original texts

        Returns:
            Dictionary mapping original text -> translation
        """
        translations = {}
        lines = response.strip().split("\n")

        for line in lines:
            line = line.strip()
            if ":" in line:
                # Extract number and translation
                match = re.match(r"(\d+):\s*(.+)", line)
                if match:
                    num = int(match.group(1))
                    translation = match.group(2).strip()

                    # Map back to original text
                    if 1 <= num <= len(original_texts):
                        original_text = original_texts[num - 1]
                        translations[original_text] = translation

        return translations

    def translate_batch(self, texts: List[str]) -> Dict[str, str]:
        """
        Translate a batch of unique texts.

        Args:
            texts: List of texts to translate

        Returns:
            Dictionary mapping original text -> translation

        Raises:
            TranslationError: If translation fails
        """
        if not texts:
            return {}

        prompt = self.create_translation_prompt(texts)

        try:
            response = self.client.models.generate_content(
                model=self.model,
                config=types.GenerateContentConfig(
                    system_instruction=f"You are an anime subtitle translator. Translate faithfully to {self.target_language} while maintaining anime context, honorifics, and cultural elements. Keep the numbered format.",
                ),
                contents=prompt,
            )
            return self.parse_translation_response(response.text or "", texts)
        except Exception as e:
            raise TranslationError(f"Failed to translate batch: {e}")

    def translate_unique_texts(self) -> None:
        """
        Translate all unique texts in batches, using concurrency if enabled.

        Raises:
            TranslationError: If translation fails
        """
        unique_text_list = list(self.unique_texts.values())
        total_texts = len(unique_text_list)
        batches = [
            unique_text_list[i : i + self.batch_size]
            for i in range(0, total_texts, self.batch_size)
        ]
        from concurrent.futures import ThreadPoolExecutor, as_completed

        results = []
        with ThreadPoolExecutor(max_workers=self.concurrence) as executor:
            future_to_batch = {
                executor.submit(self.translate_batch, batch): batch for batch in batches
            }
            for future in as_completed(future_to_batch):
                batch = future_to_batch[future]
                try:
                    batch_translations = future.result()
                    # Update translation cache using hash
                    for original_text, translation in batch_translations.items():
                        text_hash = self.generate_text_hash(original_text)
                        self.translations_cache[text_hash] = translation
                except Exception as e:
                    raise TranslationError(f"Failed to translate batch: {e}")

    def apply_translations(self, subtitle_file: pysubs2.SSAFile) -> pysubs2.SSAFile:
        """
        Apply translations to subtitle file.

        Args:
            subtitle_file: Original subtitle file

        Returns:
            Translated subtitle file
        """
        for i, line in enumerate(subtitle_file):
            if i in self.text_mappings:
                text_hash = self.text_mappings[i]
                if text_hash in self.translations_cache:
                    line.text = self.translations_cache[text_hash]

        return subtitle_file

    def set_animesubs_metadata(self, subs: pysubs2.SSAFile) -> None:
        """
        Set AnimeSubs metadata, fully overriding pysubs2 defaults in [Script Info].
        Remove all comment lines and set only AnimeSubs keys.
        """
        # Remove all comment and conflicting keys
        keys_to_remove = [
            k
            for k in list(subs.info.keys())
            if k.startswith(";") or k in ["Title", "ScriptType", "!Script", "!Website"]
        ]
        for k in keys_to_remove:
            subs.info.pop(k, None)
        subs.info.clear()
        subs.info["Title"] = "Translated by AnimeSubs"
        subs.info["ScriptType"] = "v4.00+"
        subs.info["!Script"] = "; Script generated by animesubs"
        subs.info["!Website"] = "; https://github.com/enrell/animesubs"

    def clean_script_info_section(self, output_path: str) -> None:
        """
        Substitui apenas os dois primeiros comentários do [Script Info] por linhas customizadas,
        mantendo todos os demais metadados originais intactos.
        """
        with open(output_path, "r", encoding="utf-8") as f:
            lines = f.readlines()
        in_script_info = False
        script_info_start = None
        script_info_end = None
        for idx, line in enumerate(lines):
            if line.strip().startswith("[Script Info]"):
                in_script_info = True
                script_info_start = idx
                continue
            if (
                in_script_info
                and line.strip().startswith("[")
                and not line.strip().startswith("[Script Info]")
            ):
                script_info_end = idx
                break
        if script_info_start is not None:
            # Identifica as linhas de comentário no início do bloco
            info_block = (
                lines[script_info_start + 1 : script_info_end]
                if script_info_end
                else lines[script_info_start + 1 :]
            )
            new_info_block = []
            replaced = 0
            for line in info_block:
                if line.strip().startswith(";") and replaced < 2:
                    if replaced == 0:
                        new_info_block.append("; Script generated by animesubs\n")
                    elif replaced == 1:
                        new_info_block.append("; http://github.com/enrell/animesubs\n")
                    replaced += 1
                else:
                    new_info_block.append(line)
            # Se não havia dois comentários, adiciona os faltantes
            while replaced < 2:
                if replaced == 0:
                    new_info_block.insert(0, "; Script generated by animesubs\n")
                elif replaced == 1:
                    new_info_block.insert(1, "; http://github.com/enrell/animesubs\n")
                replaced += 1
            # Reconstrói o arquivo
            new_lines = lines[: script_info_start + 1] + new_info_block
            if script_info_end:
                new_lines += lines[script_info_end:]
            with open(output_path, "w", encoding="utf-8") as f:
                f.writelines(new_lines)

    def translate_subtitle_file(self, input_path: str, output_path: str) -> bool:
        """
        Translate a complete subtitle file.

        Args:
            input_path: Input file path
            output_path: Output file path

        Returns:
            True if successful, False otherwise

        Raises:
            TranslationError: If translation process fails
        """
        try:
            # Load subtitle file
            subs = pysubs2.load(input_path)

            # Extract unique texts
            self.extract_unique_texts(subs)

            # Translate unique texts
            self.translate_unique_texts()

            # Apply translations
            translated_subs = self.apply_translations(subs)

            # Salva o arquivo sem alterar os metadados originais
            translated_subs.save(output_path)
            # Pós-processa para trocar apenas os comentários
            self.clean_script_info_section(output_path)
            return True
        except Exception as e:
            raise TranslationError(f"Failed to translate subtitle file: {e}")

    def translate_text_list(self, texts: List[str]) -> List[str]:
        """
        Translate a list of texts directly.

        Args:
            texts: List of texts to translate

        Returns:
            List of translated texts

        Raises:
            TranslationError: If translation fails
        """
        if not texts:
            return []

        # Create temporary mapping for this translation
        temp_unique_texts = {}
        temp_translations_cache = {}

        # Process texts
        for text in texts:
            clean_text = self.clean_subtitle_text(text)
            if clean_text:
                text_hash = self.generate_text_hash(clean_text)
                temp_unique_texts[text_hash] = clean_text

        # Translate in batches
        unique_text_list = list(temp_unique_texts.values())

        for i in range(0, len(unique_text_list), self.batch_size):
            batch = unique_text_list[i : i + self.batch_size]
            batch_translations = self.translate_batch(batch)

            # Update temporary cache
            for original_text, translation in batch_translations.items():
                text_hash = self.generate_text_hash(original_text)
                temp_translations_cache[text_hash] = translation

        # Apply translations to original list
        result = []
        for text in texts:
            clean_text = self.clean_subtitle_text(text)
            if clean_text:
                text_hash = self.generate_text_hash(clean_text)
                if text_hash in temp_translations_cache:
                    result.append(temp_translations_cache[text_hash])
                else:
                    result.append(text)  # Keep original if translation failed
            else:
                result.append(text)  # Keep original if not meaningful

        return result
