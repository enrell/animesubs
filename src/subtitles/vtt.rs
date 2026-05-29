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
/// use psyche_subtitle_toolkit::subtitles::vtt::VttSubtitle;
///
/// let input = "WEBVTT\n\n1\n00:00:01.000 --> 00:00:02.000\nHello world\n\n2\n00:00:03.000 --> 00:00:04.000\nGoodbye\n";
/// let vtt = VttSubtitle::parse(input).unwrap();
/// assert_eq!(vtt.document().cues.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct VttSubtitle {
    header: String,
    cues: Vec<VttCue>,
    document: SubtitleDocument,
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
        let input = input.trim_start();

        if !input.starts_with("WEBVTT") {
            return Err(SubtitleToolkitError::VttParse {
                message: "missing WEBVTT header".to_string(),
            });
        }

        // Split header from body
        let body_start = input.find("\n\n").unwrap_or(input.len());
        let header = input[..body_start].to_string();
        let body = if body_start < input.len() {
            &input[body_start + 2..]
        } else {
            ""
        };

        let mut cues = Vec::new();
        let mut doc_cues = Vec::new();
        let mut next_id = 1;

        // Split on blank lines
        let blocks: Vec<&str> = body.split("\n\n").collect();

        for block in &blocks {
            let trimmed = block.trim();
            if trimmed.is_empty() || trimmed.starts_with("NOTE") {
                continue;
            }

            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.is_empty() {
                continue;
            }

            // Find the timestamp line (contains -->)
            let mut timestamp_idx = 0;
            let mut identifier = None;

            for (i, line) in lines.iter().enumerate() {
                if line.contains("-->") {
                    timestamp_idx = i;
                    // If the first line is not the timestamp, it's an identifier
                    if i > 0 {
                        identifier = Some(lines[0].to_string());
                    }
                    break;
                }
            }

            let timestamp_line = lines[timestamp_idx].trim();
            if !timestamp_line.contains("-->") {
                continue;
            }

            // Text lines are after the timestamp
            let text = if timestamp_idx + 1 < lines.len() {
                lines[timestamp_idx + 1..].join("\n")
            } else {
                continue; // skip blocks with no text
            };

            let id = next_id;
            next_id += 1;

            cues.push(VttCue {
                id,
                identifier,
                timestamp: timestamp_line.to_string(),
                text: text.clone(),
            });
            doc_cues.push(SubtitleCue { id, text });
        }

        Ok(Self {
            header,
            cues,
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

        for (i, cue) in self.cues.iter().enumerate() {
            if i > 0 {
                output.push('\n');
            }

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
    fn skips_note_blocks() {
        let input = "WEBVTT\n\nNOTE\nThis is a note\n\n1\n00:00:01.000 --> 00:00:02.000\nHello\n";
        let vtt = VttSubtitle::parse(input).unwrap();
        assert_eq!(vtt.document().cues.len(), 1);
    }
}
