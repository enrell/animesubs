use super::TranslationRequest;

pub fn build_translation_prompt(request: &TranslationRequest<'_>) -> String {
    format!(
        "Translate the following subtitle dialogue to {target_language}.\n\nRules:\n- Preserve every numeric tag exactly, like <1>, <2>, <3>.\n- Return only translated subtitle lines.\n- Do not add explanations, markdown, notes, or code fences.\n- Keep line breaks inside each subtitle when needed.\n- Do not add curly-brace commands or backslash formatting.\n\nSubtitle dialogue:\n{source_text}",
        target_language = request.target_language,
        source_text = request.source_text,
    )
}
