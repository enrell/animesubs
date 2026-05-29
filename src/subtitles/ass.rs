use crate::error::{Result, SubtitleToolkitError};

use super::model::{SubtitleCue, SubtitleDocument};

/// A parsed ASS/SSA subtitle file.
///
/// Preserves the original file structure (headers, styles, non-dialogue lines)
/// and allows modifying dialogue text through the [`SubtitleDocument`] API.
#[derive(Debug, Clone)]
pub struct AssSubtitle {
    lines: Vec<AssLine>,
    document: SubtitleDocument,
}

impl AssSubtitle {
    /// Parse an ASS/SSA subtitle from a string.
    ///
    /// Reads the `[Events]` section, parses the `Format:` line to locate the
    /// `Text` field, and extracts each `Dialogue:` line. Non-dialogue content
    /// (headers, styles, comments) is preserved for round-trip rendering.
    ///
    /// Cue IDs are assigned sequentially starting from 1.
    pub fn parse(input: &str) -> Result<Self> {
        let mut lines = Vec::new();
        let mut in_events = false;
        let mut event_fields = None;
        let mut cues = Vec::new();
        let mut next_id = 1;

        for raw_line in input.lines() {
            let line = raw_line.to_string();
            let trimmed = line.trim();

            if trimmed.eq_ignore_ascii_case("[Events]") {
                in_events = true;
                lines.push(AssLine::Raw(line));
                continue;
            }

            if trimmed.starts_with('[') && !trimmed.eq_ignore_ascii_case("[Events]") {
                in_events = false;
                lines.push(AssLine::Raw(line));
                continue;
            }

            if in_events && trimmed.to_ascii_lowercase().starts_with("format:") {
                let fields = parse_format_fields(trimmed)?;
                event_fields = Some(fields);
                lines.push(AssLine::Raw(line));
                continue;
            }

            if in_events && trimmed.to_ascii_lowercase().starts_with("dialogue:") {
                let fields =
                    event_fields
                        .as_ref()
                        .ok_or_else(|| SubtitleToolkitError::AssParse {
                            message: "Dialogue line found before Events Format line".into(),
                        })?;
                let text_index = fields
                    .iter()
                    .position(|field| field.eq_ignore_ascii_case("text"))
                    .ok_or_else(|| SubtitleToolkitError::AssParse {
                        message: "Events Format line does not contain Text field".into(),
                    })?;
                let dialogue = parse_dialogue_line(&line, fields.len(), text_index)?;
                let id = next_id;
                next_id += 1;
                cues.push(SubtitleCue {
                    id,
                    text: decode_ass_text(&dialogue.text),
                });
                lines.push(AssLine::Dialogue { id, dialogue });
                continue;
            }

            lines.push(AssLine::Raw(line));
        }

        Ok(Self {
            lines,
            document: SubtitleDocument { cues },
        })
    }

    /// Returns a reference to the parsed subtitle document.
    pub fn document(&self) -> &SubtitleDocument {
        &self.document
    }

    /// Returns a mutable reference to the parsed subtitle document.
    pub fn document_mut(&mut self) -> &mut SubtitleDocument {
        &mut self.document
    }

    /// Render the subtitle back to an ASS string.
    ///
    /// Dialogue lines use the current text from the document; all other
    /// content (headers, styles, comments) is written as-is.
    pub fn render(&self) -> String {
        let mut rendered = Vec::with_capacity(self.lines.len());

        for line in &self.lines {
            match line {
                AssLine::Raw(raw) => rendered.push(raw.clone()),
                AssLine::Dialogue { id, dialogue } => {
                    let text = self
                        .document
                        .cues
                        .iter()
                        .find(|cue| cue.id == *id)
                        .map(|cue| encode_ass_text(&cue.text))
                        .unwrap_or_else(|| dialogue.text.clone());
                    rendered.push(dialogue.render(&text));
                }
            }
        }

        rendered.join("\n")
    }
}

#[derive(Debug, Clone)]
enum AssLine {
    Raw(String),
    Dialogue { id: usize, dialogue: AssDialogue },
}

#[derive(Debug, Clone)]
struct AssDialogue {
    prefix: String,
    fields_before_text: Vec<String>,
    text: String,
}

impl AssDialogue {
    fn render(&self, text: &str) -> String {
        let mut fields = self.fields_before_text.clone();
        fields.push(text.to_string());
        format!("{}{}", self.prefix, fields.join(","))
    }
}

fn parse_format_fields(line: &str) -> Result<Vec<String>> {
    let (_, fields) = line
        .split_once(':')
        .ok_or_else(|| SubtitleToolkitError::AssParse {
            message: format!("malformed format line: {line}"),
        })?;

    let parsed = fields
        .split(',')
        .map(|field| field.trim().to_string())
        .collect::<Vec<_>>();

    if parsed.is_empty() {
        return Err(SubtitleToolkitError::AssParse {
            message: "empty Events Format line".into(),
        });
    }

    Ok(parsed)
}

fn parse_dialogue_line(line: &str, field_count: usize, text_index: usize) -> Result<AssDialogue> {
    let (prefix, value) = line
        .split_once(':')
        .ok_or_else(|| SubtitleToolkitError::AssParse {
            message: format!("malformed dialogue line: {line}"),
        })?;
    let prefix = format!("{prefix}:");
    let value = value.strip_prefix(' ').unwrap_or(value);
    let mut fields = value
        .splitn(field_count, ',')
        .map(ToString::to_string)
        .collect::<Vec<_>>();

    if fields.len() != field_count {
        return Err(SubtitleToolkitError::AssParse {
            message: format!("dialogue field count mismatch: {line}"),
        });
    }

    let text = fields.split_off(text_index).join(",");
    Ok(AssDialogue {
        prefix,
        fields_before_text: fields,
        text,
    })
}

fn decode_ass_text(text: &str) -> String {
    text.replace("\\N", "\n").replace("\\n", "\n")
}

fn encode_ass_text(text: &str) -> String {
    text.replace('\n', "\\N")
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = "[Script Info]\nTitle: Test\n\n[Events]\nFormat: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\nDialogue: 0,0:00:01.00,0:00:02.00,Default,,0,0,0,,Hello\\Nworld";

    #[test]
    fn parses_ass_dialogue() {
        let ass = AssSubtitle::parse(SAMPLE).unwrap();

        assert_eq!(ass.document().cues.len(), 1);
        assert_eq!(ass.document().cues[0].text, "Hello\nworld");
    }

    #[test]
    fn replaces_dialogue_text() {
        let mut ass = AssSubtitle::parse(SAMPLE).unwrap();
        ass.document_mut().replace_text(1, "Olá\nmundo".into());

        assert!(ass.render().contains("Olá\\Nmundo"));
    }
}
