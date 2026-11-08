use crate::error::{Result, SubtitleToolkitError};

use super::model::{SubtitleCue, SubtitleDocument};

/// A parsed SRT (SubRip) subtitle file.
///
/// Preserves timestamps for round-trip rendering. Use [`document()`](SrtSubtitle::document)
/// to access the parsed cues for translation, and [`render()`](SrtSubtitle::render)
/// to write the translated subtitle back.
///
/// # Example
///
/// ```
/// use animesubs_engine::subtitles::srt::SrtSubtitle;
///
/// let input = "1\n00:00:01,000 --> 00:00:02,000\nHello world\n\n2\n00:00:03,000 --> 00:00:04,000\nGoodbye world\n";
/// let srt = SrtSubtitle::parse(input).unwrap();
/// assert_eq!(srt.document().cues.len(), 2);
/// ```
#[derive(Debug, Clone)]
pub struct SrtSubtitle {
    cues: Vec<SrtCue>,
    document: SubtitleDocument,
}

#[derive(Debug, Clone)]
struct SrtCue {
    id: usize,
    timestamp: String, // "00:00:01,000 --> 00:00:02,000"
    text: String,
}

impl SrtSubtitle {
    /// Parse an SRT subtitle from a string.
    ///
    /// SRT format: blocks separated by blank lines, each block containing
    /// an index line, a timestamp line (`HH:MM:SS,mmm --> HH:MM:SS,mmm`),
    /// and one or more text lines.
    ///
    /// Cue IDs are assigned sequentially starting from 1.
    pub fn parse(input: &str) -> Result<Self> {
        let normalized = input.replace("\r\n", "\n").replace('\r', "\n");
        let mut cues = Vec::new();
        let mut doc_cues = Vec::new();
        let mut next_id = 1;

        let blocks = split_blank_line_blocks(&normalized);

        for block in &blocks {
            let trimmed = block.trim();
            if trimmed.is_empty() {
                continue;
            }

            let lines: Vec<&str> = trimmed.lines().collect();
            if lines.len() < 2 {
                continue;
            }

            // First line should be the index number
            let index_line = lines[0].trim();
            let _parsed_index: usize =
                index_line
                    .parse()
                    .map_err(|_| SubtitleToolkitError::SrtParse {
                        message: format!("expected numeric index, got: {index_line}"),
                    })?;

            // Second line should be the timestamp
            let timestamp = lines[1].trim();
            if !timestamp.contains("-->") {
                return Err(SubtitleToolkitError::SrtParse {
                    message: format!("expected timestamp line, got: {timestamp}"),
                });
            }

            // Remaining lines are the subtitle text
            let text = lines[2..].join("\n");
            if text.trim().is_empty() {
                return Err(SubtitleToolkitError::SrtParse {
                    message: format!("cue {index_line} contains no subtitle text"),
                });
            }

            let id = next_id;
            next_id += 1;

            cues.push(SrtCue {
                id,
                timestamp: timestamp.to_string(),
                text: text.clone(),
            });
            doc_cues.push(SubtitleCue { id, text });
        }

        Ok(Self {
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

    /// Render the subtitle back to SRT format.
    ///
    /// Uses translated text from the document for each cue ID.
    pub fn render(&self) -> String {
        let mut output = String::new();

        for (i, cue) in self.cues.iter().enumerate() {
            if i > 0 {
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

            output.push_str(&format!("{}\n{}\n{}\n", cue.id, cue.timestamp, text));
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
        let input = "1\n00:00:01,000 --> 00:00:02,000\nHello world\n";
        let srt = SrtSubtitle::parse(input).unwrap();
        assert_eq!(srt.document().cues.len(), 1);
        assert_eq!(srt.document().cues[0].id, 1);
        assert_eq!(srt.document().cues[0].text, "Hello world");
    }

    #[test]
    fn parses_multiple_cues() {
        let input =
            "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n2\n00:00:03,000 --> 00:00:04,000\nWorld\n";
        let srt = SrtSubtitle::parse(input).unwrap();
        assert_eq!(srt.document().cues.len(), 2);
        assert_eq!(srt.document().cues[0].text, "Hello");
        assert_eq!(srt.document().cues[1].text, "World");
    }

    #[test]
    fn parses_multiline_cues() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nLine one\nLine two\n";
        let srt = SrtSubtitle::parse(input).unwrap();
        assert_eq!(srt.document().cues.len(), 1);
        assert_eq!(srt.document().cues[0].text, "Line one\nLine two");
    }

    #[test]
    fn parses_windows_crlf_blocks() {
        let input = "1\r\n00:00:01,000 --> 00:00:02,000\r\nLine one\r\n\
            Line two\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\n\
            World\r\n";
        let srt = SrtSubtitle::parse(input).unwrap();
        assert_eq!(srt.document().cues.len(), 2);
        assert_eq!(srt.document().cues[0].text, "Line one\nLine two");
        assert!(srt.render().contains("Line one\nLine two"));
    }

    #[test]
    fn renders_round_trip() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nHello world\n\n2\n00:00:03,000 --> 00:00:04,000\nGoodbye\n";
        let srt = SrtSubtitle::parse(input).unwrap();
        let rendered = srt.render();
        assert!(rendered.contains("Hello world"));
        assert!(rendered.contains("Goodbye"));
        assert!(rendered.contains("00:00:01,000 --> 00:00:02,000"));
    }

    #[test]
    fn render_uses_translated_text() {
        let input =
            "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n2\n00:00:03,000 --> 00:00:04,000\nWorld\n";
        let mut srt = SrtSubtitle::parse(input).unwrap();
        srt.document_mut().replace_text(1, "Olá".to_string());
        srt.document_mut().replace_text(2, "Mundo".to_string());
        let rendered = srt.render();
        assert!(rendered.contains("Olá"));
        assert!(rendered.contains("Mundo"));
        assert!(!rendered.contains("Hello"));
        assert!(!rendered.contains("World"));
    }

    #[test]
    fn error_on_missing_timestamp() {
        let input = "1\nnot a timestamp\nHello\n";
        let err = SrtSubtitle::parse(input).unwrap_err();
        assert!(err.to_string().contains("timestamp"));
    }

    #[test]
    fn error_on_non_numeric_index() {
        let input = "abc\n00:00:01,000 --> 00:00:02,000\nHello\n";
        let err = SrtSubtitle::parse(input).unwrap_err();
        assert!(err.to_string().contains("numeric index"));
    }

    #[test]
    fn skips_empty_blocks() {
        let input = "1\n00:00:01,000 --> 00:00:02,000\nHello\n\n\n\n2\n00:00:03,000 --> 00:00:04,000\nWorld\n";
        let srt = SrtSubtitle::parse(input).unwrap();
        assert_eq!(srt.document().cues.len(), 2);
    }
}
