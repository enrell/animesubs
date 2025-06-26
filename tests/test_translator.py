"""
Tests for SubtitleTranslator class.
"""

from unittest.mock import Mock, patch

import pysubs2
import pytest

from animesubs import SubtitleTranslator, TranslationError


class TestSubtitleTranslator:
    """Test cases for SubtitleTranslator class."""

    def test_init_with_valid_api_key(self, api_key, target_language):
        """Test translator initialization with valid API key."""
        with patch("animesubs.translator.genai.Client") as mock_client:
            translator = SubtitleTranslator(
                api_key=api_key, target_language=target_language, batch_size=25
            )

            assert translator.target_language == target_language
            assert translator.batch_size == 25
            assert translator.model == "gemini-2.5-flash-lite-preview-06-17"
            mock_client.assert_called_once_with(api_key=api_key)

    def test_init_with_invalid_api_key(self):
        """Test translator initialization with invalid API key."""
        with patch(
            "animesubs.translator.genai.Client",
            side_effect=Exception("Invalid API key"),
        ):
            with pytest.raises(
                TranslationError, match="Failed to initialize Gemini client"
            ):
                SubtitleTranslator(api_key="invalid_key")

    def test_is_technical_command(self, api_key):
        """Test technical command detection."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key)

            # Technical commands should return True
            assert translator.is_technical_command("m 0 0 l 150 0 150 150 0 150")
            assert translator.is_technical_command("m 100 200 l 300 400")
            assert translator.is_technical_command("\\move(100,200,300,400)")
            assert translator.is_technical_command("123 456 789 012")

            # Regular text should return False
            assert not translator.is_technical_command("Hello, world!")
            assert not translator.is_technical_command("Konnichiwa!")
            assert not translator.is_technical_command("123")  # Short numbers

    def test_is_meaningful_text(self, api_key):
        """Test meaningful text detection."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key)

            # Meaningful text should return True
            assert translator.is_meaningful_text("Hello, world!")
            assert translator.is_meaningful_text("Konnichiwa, Tanaka-san!")
            assert translator.is_meaningful_text("This is a test.")

            # Non-meaningful text should return False
            assert not translator.is_meaningful_text("ah")  # Too short
            assert not translator.is_meaningful_text("...")  # Only symbols
            assert not translator.is_meaningful_text("aaa")  # Repeated letters
            assert not translator.is_meaningful_text("hmm")  # Short interjection
            assert not translator.is_meaningful_text("ing")  # Word fragment

    def test_clean_subtitle_text(self, api_key):
        """Test subtitle text cleaning."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key)

            # Test tag removal
            assert translator.clean_subtitle_text("{\\an8}Hello") == "Hello"
            assert translator.clean_subtitle_text("<b>World</b>") == "World"
            assert translator.clean_subtitle_text("Hello\\NWorld") == "Hello World"

            # Test technical command filtering
            assert translator.clean_subtitle_text("m 0 0 l 150 0") == ""

            # Test meaningful text preservation
            assert (
                translator.clean_subtitle_text("Hello, Tanaka-san!")
                == "Hello, Tanaka-san!"
            )

            # Test empty/whitespace handling
            assert translator.clean_subtitle_text("") == ""
            assert translator.clean_subtitle_text("   ") == ""

    def test_generate_text_hash(self, api_key):
        """Test text hash generation."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key)

            text1 = "Hello, world!"
            text2 = "Hello, world!"
            text3 = "Different text"

            hash1 = translator.generate_text_hash(text1)
            hash2 = translator.generate_text_hash(text2)
            hash3 = translator.generate_text_hash(text3)

            # Same text should produce same hash
            assert hash1 == hash2
            # Different text should produce different hash
            assert hash1 != hash3
            # Hash should be consistent
            assert len(hash1) == 32  # MD5 hash length

    def test_create_translation_prompt(self, api_key, sample_texts):
        """Test translation prompt creation."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key, target_language="pt-BR")

            prompt = translator.create_translation_prompt(sample_texts)

            # Check prompt contains anime-specific guidelines
            assert "ANIME TRANSLATION GUIDELINES" in prompt
            assert "-san, -sama, -kun, -chan" in prompt
            assert "onii-chan, onee-chan" in prompt
            assert "pt-BR" in prompt

            # Check numbered format
            for i, text in enumerate(sample_texts, 1):
                assert f"{i}: {text}" in prompt

    def test_parse_translation_response(self, api_key, sample_texts):
        """Test translation response parsing."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key)

            response = """1: Olá, como você está?
2: Estou bem, obrigado.
3: Qual é o seu nome?"""

            translations = translator.parse_translation_response(
                response, sample_texts[:3]
            )

            assert len(translations) == 3
            assert translations[sample_texts[0]] == "Olá, como você está?"
            assert translations[sample_texts[1]] == "Estou bem, obrigado."
            assert translations[sample_texts[2]] == "Qual é o seu nome?"

    def test_translate_batch_success(self, api_key, sample_texts, mock_genai_client):
        """Test successful batch translation."""
        with patch("animesubs.translator.genai.Client", return_value=mock_genai_client):
            translator = SubtitleTranslator(api_key=api_key)

            translations = translator.translate_batch(sample_texts[:3])

            assert len(translations) == 3
            mock_genai_client.models.generate_content.assert_called_once()

    def test_translate_batch_failure(self, api_key, sample_texts):
        """Test batch translation failure."""
        mock_client = Mock()
        mock_client.models.generate_content.side_effect = Exception("API Error")

        with patch("animesubs.translator.genai.Client", return_value=mock_client):
            translator = SubtitleTranslator(api_key=api_key)

            with pytest.raises(TranslationError, match="Failed to translate batch"):
                translator.translate_batch(sample_texts)

    def test_translate_batch_empty_list(self, api_key):
        """Test batch translation with empty list."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key)

            result = translator.translate_batch([])
            assert result == {}

    def test_extract_unique_texts(self, api_key, sample_subtitle_file):
        """Test unique text extraction from subtitle file."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key)

            subs = pysubs2.load(str(sample_subtitle_file))
            unique_texts_map = translator.extract_unique_texts(subs)

            # Should have filtered out technical commands
            assert len(translator.unique_texts) < len(subs)
            assert len(unique_texts_map) == len(translator.unique_texts)

    def test_translate_text_list(self, api_key, sample_texts, mock_genai_client):
        """Test direct text list translation."""
        with patch("animesubs.translator.genai.Client", return_value=mock_genai_client):
            translator = SubtitleTranslator(api_key=api_key)

            result = translator.translate_text_list(sample_texts[:3])

            assert len(result) == 3
            assert all(isinstance(text, str) for text in result)

    def test_translate_subtitle_file_success(
        self, api_key, sample_subtitle_file, temp_dir, mock_genai_client
    ):
        """Test successful subtitle file translation."""
        with patch("animesubs.translator.genai.Client", return_value=mock_genai_client):
            translator = SubtitleTranslator(api_key=api_key)

            output_path = temp_dir / "translated.ass"

            success = translator.translate_subtitle_file(
                str(sample_subtitle_file), str(output_path)
            )

            assert success is True
            assert output_path.exists()

            # Check output file has script info
            with open(output_path, "r", encoding="utf-8") as f:
                content = f.read()
                assert "animesubs" in content

    def test_translate_subtitle_file_not_found(self, api_key, temp_dir):
        """Test subtitle file translation with non-existent input file."""
        with patch("animesubs.translator.genai.Client"):
            translator = SubtitleTranslator(api_key=api_key)

            non_existent_file = temp_dir / "non_existent.ass"
            output_path = temp_dir / "output.ass"

            with pytest.raises(TranslationError):
                translator.translate_subtitle_file(
                    str(non_existent_file), str(output_path)
                )
