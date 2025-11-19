import hashlib
import re
from typing import Dict, List, Optional, Tuple

import pysubs2
from concurrent.futures import ThreadPoolExecutor, as_completed

from .exceptions import TranslationError
from .text_utils import clean_subtitle_text
from .providers import get_provider


class SubtitleTranslator:
    def __init__(
        self,
        api_key: Optional[str] = None,
        source_language: str = "Japanese",
        target_language: str = "pt-BR",
        batch_size: int = 300,
        concurrence: int = 1,
        translations_cache: Optional[Dict[str, str]] = None,
        provider: str = "gemini",
        **provider_kwargs,
    ):
        self.source_language = source_language
        self.target_language = target_language
        self.batch_size = batch_size
        self.concurrence = concurrence
        self.api_key = api_key
        self.provider_name = provider
        self.provider_kwargs = provider_kwargs
        self.translations_cache = (
            translations_cache if translations_cache is not None else {}
        )

        try:
            self.provider = get_provider(provider, api_key=api_key, **provider_kwargs)
        except Exception as e:
            raise TranslationError(f"Failed to initialize provider '{provider}': {e}")

    def generate_text_hash(self, text: str) -> str:
        return hashlib.md5(text.encode("utf-8")).hexdigest()

    def extract_unique_texts(
        self, subtitle_file: pysubs2.SSAFile
    ) -> Tuple[Dict[str, str], Dict[int, str]]:
        unique_texts = {}
        text_mappings = {}

        for i, line in enumerate(subtitle_file):
            original_text = line.text
            clean_text = clean_subtitle_text(original_text)

            if not clean_text:
                continue

            text_hash = self.generate_text_hash(clean_text)
            unique_texts[text_hash] = clean_text
            text_mappings[i] = text_hash

        return unique_texts, text_mappings

    def create_translation_prompt(self, texts: List[str]) -> str:
        numbered_texts = []
        for i, text in enumerate(texts, 1):
            numbered_texts.append(f"{i}: {text}")

        prompt = f"""Translate the following anime subtitles from {self.source_language} to {self.target_language}. 
Keep the same numbered format. Preserve honorifics, cultural terms, names, and anime tone.
Don't add explanations, just translate:

{chr(10).join(numbered_texts)}

Response:"""
        return prompt
        return prompt

    def parse_translation_response(
        self, response: str, original_texts: List[str]
    ) -> Dict[str, str]:
        translations = {}
        lines = response.strip().split("\n")

        for line in lines:
            line = line.strip()
            if ":" in line:
                match = re.match(r"(\d+):\s*(.+)", line)
                if match:
                    num = int(match.group(1))
                    translation = match.group(2).strip()
                    if 1 <= num <= len(original_texts):
                        original_text = original_texts[num - 1]
                        translations[original_text] = translation
        return translations

    def translate_batch(self, texts: List[str]) -> Dict[str, str]:
        if not texts:
            return {}
        prompt = self.create_translation_prompt(texts)
        try:
            response_text = self.provider.translate_batch(
                texts, self.target_language, prompt
            )
            return self.parse_translation_response(response_text, texts)
        except Exception as e:
            raise TranslationError(f"Failed to translate batch: {e}")

    def translate_missing_texts(self, unique_texts: Dict[str, str]) -> None:
        texts_to_translate = []
        for text_hash, text in unique_texts.items():
            if text_hash not in self.translations_cache:
                texts_to_translate.append(text)

        if not texts_to_translate:
            return

        batches = [
            texts_to_translate[i : i + self.batch_size]
            for i in range(0, len(texts_to_translate), self.batch_size)
        ]

        with ThreadPoolExecutor(max_workers=self.concurrence) as executor:
            future_to_batch = {
                executor.submit(self.translate_batch, batch): batch for batch in batches
            }
            for future in as_completed(future_to_batch):
                try:
                    batch_translations = future.result()
                    for original_text, translation in batch_translations.items():
                        text_hash = self.generate_text_hash(original_text)
                        self.translations_cache[text_hash] = translation
                except Exception as e:
                    raise TranslationError(f"Failed to translate batch: {e}")

    def apply_translations(
        self, subtitle_file: pysubs2.SSAFile, text_mappings: Dict[int, str]
    ) -> pysubs2.SSAFile:
        for i, line in enumerate(subtitle_file):
            if i in text_mappings:
                text_hash = text_mappings[i]
                if text_hash in self.translations_cache:
                    translation = self.translations_cache[text_hash]
                    if translation:
                        # Try to preserve tags at the beginning
                        tags_match = re.match(r"^([^}]+\})+", line.text)
                        if tags_match:
                            tags = tags_match.group(1)
                            line.text = f"{tags}{translation}"
                        else:
                            line.text = translation
        return subtitle_file

    def set_animesubs_metadata(self, subs: pysubs2.SSAFile) -> None:
        keys_to_remove = [
            k
            for k in list(subs.info.keys())
            if k.startswith(";") or k in ["Title", "ScriptType", "!Script", "!Website"]
        ]
        for k in keys_to_remove:
            subs.info.pop(k, None)
        subs.info["Title"] = "Translated by AnimeSubs"
        subs.info["ScriptType"] = "v4.00+"
        subs.info["!Script"] = "; Script generated by animesubs"
        subs.info["!Website"] = "; https://github.com/enrell/animesubs"

    def clean_script_info_section(self, output_path: str) -> None:
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
            while replaced < 2:
                if replaced == 0:
                    new_info_block.insert(0, "; Script generated by animesubs\n")
                elif replaced == 1:
                    new_info_block.insert(1, "; http://github.com/enrell/animesubs\n")
                replaced += 1
            new_lines = lines[: script_info_start + 1] + new_info_block
            if script_info_end:
                new_lines += lines[script_info_end:]
            with open(output_path, "w", encoding="utf-8") as f:
                f.writelines(new_lines)

    def translate_subtitle_file(self, input_path: str, output_path: str) -> bool:
        try:
            subs = pysubs2.load(input_path)
            unique_texts, text_mappings = self.extract_unique_texts(subs)
            self.translate_missing_texts(unique_texts)
            translated_subs = self.apply_translations(subs, text_mappings)
            translated_subs.save(output_path)
            self.clean_script_info_section(output_path)
            return True
        except Exception as e:
            raise TranslationError(f"Failed to translate subtitle file: {e}")

    def translate_text_list(self, texts: List[str]) -> List[str]:
        if not texts:
            return []

        temp_unique_texts = {}
        for text in texts:
            clean_text = clean_subtitle_text(text)
            if clean_text:
                text_hash = self.generate_text_hash(clean_text)
                temp_unique_texts[text_hash] = clean_text

        self.translate_missing_texts(temp_unique_texts)

        result = []
        for text in texts:
            clean_text = clean_subtitle_text(text)
            if clean_text:
                text_hash = self.generate_text_hash(clean_text)
                if text_hash in self.translations_cache:
                    result.append(self.translations_cache[text_hash])
                else:
                    result.append(text)
            else:
                result.append(text)
        return result
