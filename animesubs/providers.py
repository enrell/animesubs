from abc import ABC, abstractmethod
from typing import Dict, List, Optional, Any, cast
import os

from google import genai
from google.genai import types
import openai
import ollama

try:
    import lmstudio
except ImportError:
    lmstudio = None

from .exceptions import TranslationError


class TranslationProvider(ABC):
    @abstractmethod
    def translate_batch(
        self, texts: List[str], target_language: str, prompt_template: str
    ) -> str:
        """
        Translates a batch of texts.
        Returns the raw response text from the model.
        """
        pass


class GeminiProvider(TranslationProvider):
    def __init__(self, api_key: str, model: str = "gemini-flash-latest"):
        self.client = genai.Client(api_key=api_key)
        self.model = model

    def translate_batch(
        self, texts: List[str], target_language: str, prompt_template: str
    ) -> str:
        try:
            response = self.client.models.generate_content(
                model=self.model,
                config=types.GenerateContentConfig(
                    system_instruction=f"You are an anime subtitle translator. Translate faithfully to {target_language} while maintaining anime context, honorifics, and cultural elements. Keep the numbered format.",
                ),
                contents=prompt_template,
            )
            return response.text or ""
        except Exception as e:
            raise TranslationError(f"Gemini translation failed: {e}")


class OpenAIProvider(TranslationProvider):
    def __init__(
        self, api_key: str, base_url: Optional[str] = None, model: str = "gpt-4o-mini"
    ):
        self.client = openai.OpenAI(api_key=api_key, base_url=base_url)
        self.model = model

    def translate_batch(
        self, texts: List[str], target_language: str, prompt_template: str
    ) -> str:
        try:
            response = self.client.chat.completions.create(
                model=self.model,
                messages=[
                    {
                        "role": "system",
                        "content": f"You are an anime subtitle translator. Translate faithfully to {target_language} while maintaining anime context, honorifics, and cultural elements. Keep the numbered format.",
                    },
                    {"role": "user", "content": prompt_template},
                ],
            )
            return response.choices[0].message.content or ""
        except Exception as e:
            raise TranslationError(f"OpenAI translation failed: {e}")


class OllamaProvider(TranslationProvider):
    def __init__(self, model: str = "llama3", host: Optional[str] = None):
        self.model = model
        self.client = ollama.Client(host=host) if host else ollama.Client()

    def translate_batch(
        self, texts: List[str], target_language: str, prompt_template: str
    ) -> str:
        try:
            response = self.client.chat(
                model=self.model,
                messages=[
                    {
                        "role": "system",
                        "content": f"You are an anime subtitle translator. Translate faithfully to {target_language} while maintaining anime context, honorifics, and cultural elements. Keep the numbered format.",
                    },
                    {"role": "user", "content": prompt_template},
                ],
            )
            return response["message"]["content"]
        except Exception as e:
            raise TranslationError(f"Ollama translation failed: {e}")


class LMStudioProvider(TranslationProvider):
    def __init__(self, model: str, base_url: str = "localhost:1234"):
        if lmstudio is None:
            raise TranslationError("lmstudio package is not installed.")

        # Clean up base_url for LM Studio SDK which seems to prepend ws://
        if base_url.startswith("ws://"):
            base_url = base_url[5:]
        elif base_url.startswith("http://"):
            base_url = base_url[7:]

        try:
            self.client = lmstudio.Client(api_host=base_url)
            self.model_path = model
            self.loaded_model = None
        except Exception as e:
            raise TranslationError(f"Failed to initialize LM Studio client: {e}")

    def _get_model(self):
        if not self.loaded_model:
            try:
                # Use the model method to get a handle, assuming it might load or reference it.
                # Based on inspection: client.llm.model(path) or client.llm.load_new_instance(path)
                # Let's try model() first as it seems more generic.
                # If the model is not loaded, we might need to load it.
                # But 'model' usually implies getting a reference.
                self.loaded_model = self.client.llm.model(self.model_path)
            except Exception as e:
                raise TranslationError(
                    f"Failed to get model '{self.model_path}' in LM Studio: {e}"
                )
        return self.loaded_model

    def translate_batch(
        self, texts: List[str], target_language: str, prompt_template: str
    ) -> str:
        try:
            model = self._get_model()
            # respond takes Chat | ChatHistoryDataDict | str
            # We construct a ChatHistoryDataDict with proper content structure
            history = {
                "messages": [
                    {
                        "role": "system",
                        "content": [
                            {
                                "type": "text",
                                "text": f"You are an anime subtitle translator. Translate faithfully to {target_language} while maintaining anime context, honorifics, and cultural elements. Keep the numbered format.",
                            }
                        ],
                    },
                    {
                        "role": "user",
                        "content": [{"type": "text", "text": prompt_template}],
                    },
                ]
            }
            result = model.respond(cast(Any, history))
            return result.content
        except Exception as e:
            raise TranslationError(f"LM Studio translation failed: {e}")


def get_provider(provider_type: str, **kwargs) -> TranslationProvider:
    if provider_type == "gemini":
        api_key = kwargs.get("api_key")
        if not api_key:
            raise ValueError("API key is required for Gemini provider")
        return GeminiProvider(
            api_key=api_key, model=kwargs.get("model", "gemini-2.5-flash-lite")
        )
    elif provider_type == "openai":
        api_key = kwargs.get("api_key")
        if not api_key:
            raise ValueError("API key is required for OpenAI provider")
        return OpenAIProvider(
            api_key=api_key,
            base_url=kwargs.get("base_url"),
            model=kwargs.get("model", "gpt-4o-mini"),
        )
    elif provider_type == "ollama":
        return OllamaProvider(
            model=kwargs.get("model", "llama3"), host=kwargs.get("host")
        )
    elif provider_type == "lmstudio":
        return LMStudioProvider(
            model=kwargs.get("model", "default"),
            base_url=kwargs.get("base_url", "localhost:1234"),
        )
    else:
        raise ValueError(f"Unknown provider type: {provider_type}")
