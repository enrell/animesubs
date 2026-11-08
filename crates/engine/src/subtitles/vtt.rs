use crate::error::{Result, SubtitleToolkitError};

use super::model::{SubtitleCue, SubtitleDocument};

/// A parsed WebVTT subtitle file.
///
/// Preserves the VTT header and timestamps for round-trip rendering.
/// Use [`document()`](VttSubtitle::document) to access the parsed cues for
/// translation, and [`render()`](VttSubtitle::render) to write the translated
/// subtitle back.
///
/// # Example
///
/// ```
/// use animesubs_engine::subtitles::vtt::VttSubtitle;
///
/// let input = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHello world\n\n\
///     2\n00:00:03.000 --> 00:00:04.000\nGoodbye\n";
/// let vtt = VttSubtitle::parse(input).unwrap();
/// assert_eq!(vtt.document().cues.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct VttSubtitle {
    header: String,
    blocks: Vec<VttBlock>,
    document: SubtitleDocument,
}

#[derive(Debug, Clone)]
enum VttBlock {
    Cue(VttCue),
    Raw(String),
}

#[derive(Debug, Clone)]
struct VttCue {
    id: usize,
    identifier: Option<String>, // optional cue identifier
    timestamp: String,          // "00:00:01.000 --> 00:00:02.000"
    text: String,
}

impl VttSubtitle {
    /// Parse a WebVTT subtitle from a string.
    ///
    /// WebVTT format: starts with `WEBVTT`, then blocks separated by blank lines.
    /// Each block can have an optional identifier, a timestamp line
    /// (`HH:MM:SS.mmm --> HH:MM:SS.mmm`), and one or more text lines.
    ///
    /// Cue IDs are assigned sequentially starting from 1.
    pub fn parse(input: &str) -> Result<Self> {
        let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
        let input = normalized.trim_start_matches(['\u{feff}', ' ', '\t', '\n']);

        if !input.starts_with("WEBVTT") {
            return Err(SubtitleToolkitError::VttParse {
                message: "missing WEBVTT header".to_string(),
            });
        }

        let sections = split_blank_line_blocks(input);
        let header = sections.first().cloned().unwrap_or_default();

        let mut blocks_out = Vec::new();
        let mut doc_cues = Vec::new();
        let mut next_id = 1;

        for block in sections.iter().skip(1) {
            let trimmed = block.trim();
            if trimmed.is_empty() {
                continue;
            }

            if trimmed.starts_with("NOTE")
                || trimmed.starts_with("STYLE")
                || trimmed.starts_with("REGION")
            {
                blocks_out.push(VttBlock::Raw(trimmed.to_string()));
                continue;
            }

            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.is_empty() {
                continue;
            }

            // Find the timestamp line (contains -->)
            let (timestamp_idx, identifier) = if lines[0].contains("-->") {
                (0, None)
            } else if lines.get(1).is_some_and(|line| line.contains("-->")) {
                (1, Some(lines[0].to_string()))
            } else {
                blocks_out.push(VttBlock::Raw(trimmed.to_string()));
                continue;
            };
            let timestamp_line = lines[timestamp_idx].trim();

            // Text lines are after the timestamp
            let text = if timestamp_idx + 1 < lines.len() {
                lines[timestamp_idx + 1..].join("\n")
            } else {
                return Err(SubtitleToolkitError::VttParse {
                    message: format!("cue at {timestamp_line} contains no subtitle text"),
                });
            };

            let id = next_id;
            next_id += 1;

            blocks_out.push(VttBlock::Cue(VttCue {
                id,
                identifier,
                timestamp: timestamp_line.to_string(),
                text: text.clone(),
            }));
            doc_cues.push(SubtitleCue { id, text });
        }

        Ok(Self {
            header,
            blocks: blocks_out,
            document: SubtitleDocument { cues: doc_cues },
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

    /// Render the subtitle back to WebVTT format.
    ///
    /// Uses translated text from the document for each cue ID.
    pub fn render(&self) -> String {
        let mut output = format!("{}\n\n", self.header);

        for (i, block) in self.blocks.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }

            let VttBlock::Cue(cue) = block else {
                if let VttBlock::Raw(raw) = block {
                    output.push_str(raw);
                    output.push('\n');
                }
                continue;
            };

            // Identifier line (if present)
            if let Some(ref ident) = cue.identifier {
                output.push_str(ident);
                output.push('\n');
            }

            // Find the translated text from the document
            let text = self
                .document
                .cues
                .iter()
                .find(|c| c.id == cue.id)
                .map(|c| c.text.as_str())
                .unwrap_or(&cue.text);

            output.push_str(&format!("{}\n{}\n", cue.timestamp, text));
        }

        output
    }
}

fn split_blank_line_blocks(input: &str) -> Vec<String> {
    let mut blocks = Vec::new();
    let mut current = Vec::new();
    for line in input.lines() {
        if line.trim().is_empty() {
            if !current.is_empty() {
                blocks.push(current.join("\n"));
                current.clear();
            }
        } else {
            current.push(line);
        }
    }
    if !current.is_empty() {
        blocks.push(current.join("\n"));
    }
    blocks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_cue() {
        let input = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHello world\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        assert_eq!(vtt.document().cues.len(), 1);
        assert_eq!(vtt.document().cues[0].id, 1);
        assert_eq!(vtt.document().cues[0].text, "Hello world");
    }

    #[test]
    fn parses_multiple_cues() {
        let input = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHello\n\n2\n00:00:03.000 --> 00:00:04.000\nWorld\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        assert_eq!(vtt.document().cues.len(), 2);
        assert_eq!(vtt.document().cues[0].text, "Hello");
        assert_eq!(vtt.document().cues[1].text, "World");
    }

    #[test]
    fn parses_multiline_cues() {
        let input = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nLine one\nLine two\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        assert_eq!(vtt.document().cues.len(), 1);
        assert_eq!(vtt.document().cues[0].text, "Line one\nLine two");
    }

    #[test]
    fn parses_windows_crlf_blocks() {
        let input = "WEBVTT\r\n\r\n1\r\n00:00:01.000 --> 00:00:02.000\r\n\
            Line one\r\nLine two\r\n\r\n2\r\n00:00:03.000 --> 00:00:04.000\r\n\
            World\r\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        assert_eq!(vtt.document().cues.len(), 2);
        assert_eq!(vtt.document().cues[0].text, "Line one\nLine two");
    }

    #[test]
    fn parses_cue_identifiers() {
        let input = "WEBVTT\n\ncue-1\n00:00:01.000 --> 00:00:02.000\nHello\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        assert_eq!(vtt.document().cues.len(), 1);
        assert_eq!(vtt.document().cues[0].text, "Hello");
    }

    #[test]
    fn renders_round_trip() {
        let input = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHello\n\n2\n00:00:03.000 --> 00:00:04.000\nWorld\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        let rendered = vtt.render();
        assert!(rendered.starts_with("WEBVTT"));
        assert!(rendered.contains("Hello"));
        assert!(rendered.contains("World"));
        assert!(rendered.contains("00:00:01.000 --> 00:00:02.000"));
    }

    #[test]
    fn render_uses_translated_text() {
        let input = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHello\n\n2\n00:00:03.000 --> 00:00:04.000\nWorld\n";
        let mut vtt = VttSubtitle::parse(input).unwrap();
        vtt.document_mut().replace_text(1, "Olá".to_string());
        vtt.document_mut().replace_text(2, "Mundo".to_string());
        let rendered = vtt.render();
        assert!(rendered.contains("Olá"));
        assert!(rendered.contains("Mundo"));
        assert!(!rendered.contains("Hello"));
        assert!(!rendered.contains("World"));
    }

    #[test]
    fn renders_preserves_identifiers() {
        let input = "WEBVTT\n\ncue-1\n00:00:01.000 --> 00:00:02.000\nHello\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        let rendered = vtt.render();
        assert!(rendered.contains("cue-1"));
    }

    #[test]
    fn error_on_missing_header() {
        let input = "1\n00:00:01.000 --> 00:00:02.000\nHello\n";
        let err = VttSubtitle::parse(input).unwrap_err();
        assert!(err.to_string().contains("WEBVTT"));
    }

    #[test]
    fn preserves_note_style_and_region_blocks() {
        let input = "WEBVTT\n\nNOTE\nThis is a note\n\nSTYLE\n::cue { color: lime; }\n\
            \nREGION\nid:fred\n\n1\n00:00:01.000 --> 00:00:02.000\n\
            Hello\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        assert_eq!(vtt.document().cues.len(), 1);
        let rendered = vtt.render();
        assert!(rendered.contains("NOTE\nThis is a note"));
        assert!(rendered.contains("STYLE\n::cue { color: lime; }"));
        assert!(rendered.contains("REGION\nid:fred"));
    }
}
