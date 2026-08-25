use std::collections::{BTreeMap, HashMap, HashSet};

use crate::error::{Result, SubtitleToolkitError};

use super::model::{SubtitleCue, SubtitleDocument};

const PROTECTED_TAG_PREFIX: &str = "[[PSY_TAG_";

#[derive(Debug, Clone, Default)]
pub(crate) struct ProtectedSpanMap {
    spans: HashMap<usize, Vec<ProtectedSpan>>,
}

#[derive(Debug, Clone)]
struct ProtectedSpan {
    token: String,
    original: String,
}

/// Serialize a subtitle document to numbered text for LLM translation.
///
/// Each cue becomes `<id> text` on its own line. Internal newlines are
/// encoded as `\\N` (ASS hard break).
pub fn to_numbered_text(document: &SubtitleDocument) -> String {
    document
        .cues
        .iter()
        .map(|cue| format!("<{}> {}", cue.id, cue.text.replace('\n', "\\N")))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse numbered text from an LLM response back into a cue ID → text map.
///
/// Validates that all `expected_ids` are present, no duplicates exist,
/// and no unexpected IDs appear. Multiline continuation lines are joined
/// with `\n`.
pub fn parse_numbered_text(text: &str, expected_ids: &[usize]) -> Result<BTreeMap<usize, String>> {
    let expected: HashSet<usize> = expected_ids.iter().copied().collect();
    let mut parsed = BTreeMap::new();
    let mut current_id = None;

    for line in text.lines() {
        if let Some((id, value)) = parse_numbered_line(line)? {
            if !expected.contains(&id) {
                return Err(SubtitleToolkitError::InvalidTranslation {
                    message: format!("unexpected id <{id}>"),
                });
            }

            if parsed.insert(id, value.to_string()).is_some() {
                return Err(SubtitleToolkitError::InvalidTranslation {
                    message: format!("duplicate id <{id}>"),
                });
            }
            current_id = Some(id);
        } else if let Some(id) = current_id {
            let value = parsed.get_mut(&id).expect("current id must exist");
            if !value.is_empty() {
                value.push('\n');
            }
            value.push_str(line);
        } else if !line.trim().is_empty() {
            return Err(SubtitleToolkitError::InvalidTranslation {
                message: format!("text before first id: {line}"),
            });
        }
    }

    for id in expected_ids {
        if !parsed.contains_key(id) {
            return Err(SubtitleToolkitError::InvalidTranslation {
                message: format!("missing id <{id}>"),
            });
        }
    }

    for (id, value) in &mut parsed {
        *value = value.replace("\\N", "\n");
        if value.trim().is_empty() {
            return Err(SubtitleToolkitError::InvalidTranslation {
                message: format!("empty translation for id <{id}>"),
            });
        }
    }

    Ok(parsed)
}

fn parse_numbered_line(line: &str) -> Result<Option<(usize, &str)>> {
    let trimmed = line.trim_start();
    let Some(rest) = trimmed.strip_prefix('<') else {
        return Ok(None);
    };
    let Some((id, value)) = rest.split_once('>') else {
        return Err(SubtitleToolkitError::InvalidTranslation {
            message: format!("malformed numbered line: {line}"),
        });
    };
    let id = id
        .parse::<usize>()
        .map_err(|_| SubtitleToolkitError::InvalidTranslation {
            message: format!("invalid id in line: {line}"),
        })?;

    Ok(Some((id, value.trim_start())))
}

/// Apply translated text to a document, replacing each cue's text by ID.
pub fn apply_translation(document: &mut SubtitleDocument, translated: BTreeMap<usize, String>) {
    for (id, text) in translated {
        document.replace_text(id, text);
    }
}

fn find_tag_end(text: &str) -> usize {
    if !text.starts_with('{') {
        return 0;
    }

    let mut depth = 0;
    for (i, ch) in text.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return i + ch.len_utf8();
                }
            }
            _ => {}
        }
    }

    0
}

fn strip_override_tags(text: &str) -> (String, String) {
    let mut tags = String::new();
    let mut remaining = text;

    loop {
        let end = find_tag_end(remaining);
        if end == 0 {
            break;
        }
        tags.push_str(&remaining[..end]);
        remaining = &remaining[end..];
    }

    (remaining.to_string(), tags)
}

/// Strip leading ASS override tags (`{\\pos(...)}`, `{\\an7}`, etc.) from all cues.
///
/// Returns a clean document (tags removed) and a map of cue ID → stripped tags.
/// Use [`reinject_tags`] to restore tags after translation.
pub fn strip_tags(document: &SubtitleDocument) -> (SubtitleDocument, HashMap<usize, String>) {
    let mut tag_map = HashMap::new();
    let mut clean_cues = Vec::with_capacity(document.cues.len());

    for cue in &document.cues {
        let (clean, tags) = strip_override_tags(&cue.text);
        if !tags.is_empty() {
            tag_map.insert(cue.id, tags);
        }
        clean_cues.push(SubtitleCue {
            id: cue.id,
            text: clean,
        });
    }

    (SubtitleDocument { cues: clean_cues }, tag_map)
}

/// Restore ASS override tags previously stripped by [`strip_tags`].
///
/// Prepends stored tags back onto each cue's text.
pub fn reinject_tags(document: &mut SubtitleDocument, tag_map: &HashMap<usize, String>) {
    for cue in &mut document.cues {
        if let Some(tags) = tag_map.get(&cue.id) {
            cue.text = format!("{}{}", tags, cue.text);
        }
    }
}

/// Split a subtitle document into chunks for batched LLM translation.
///
/// Each chunk's numbered text representation will be at most `max_chars` characters.
/// Cue IDs are preserved across chunks. A single cue longer than `max_chars`
/// gets its own chunk (never split mid-cue).
pub fn chunk_document(document: &SubtitleDocument, max_chars: usize) -> Vec<SubtitleDocument> {
    if document.cues.is_empty() {
        return vec![];
    }

    let mut chunks = Vec::new();
    let mut current_cues = Vec::new();
    let mut current_chars = 0;

    for cue in &document.cues {
        let line_chars = format!("<{}> {}", cue.id, cue.text.replace('\n', "\\N")).len();

        if !current_cues.is_empty() && current_chars + 1 + line_chars > max_chars {
            chunks.push(SubtitleDocument {
                cues: std::mem::take(&mut current_cues),
            });
            current_chars = 0;
        }

        current_chars += if current_cues.is_empty() {
            line_chars
        } else {
            1 + line_chars
        };
        current_cues.push(cue.clone());
    }

    if !current_cues.is_empty() {
        chunks.push(SubtitleDocument { cues: current_cues });
    }

    chunks
}

/// Split a document into chunks of at most `max_lines` cues each.
///
/// This is an alternative to [`chunk_document`] that chunks by line count
/// instead of character count, which is more predictable for LLM token usage.
pub fn chunk_document_by_lines(
    document: &SubtitleDocument,
    max_lines: usize,
) -> Vec<SubtitleDocument> {
    if document.cues.is_empty() || max_lines == 0 {
        return vec![];
    }

    document
        .cues
        .chunks(max_lines)
        .map(|chunk| SubtitleDocument {
            cues: chunk.to_vec(),
        })
        .collect()
}

/// Split a document while respecting both item-count and UTF-8 byte limits.
pub fn chunk_document_with_limits(
    document: &SubtitleDocument,
    max_items: usize,
    max_request_bytes: usize,
) -> Result<Vec<SubtitleDocument>> {
    if max_items == 0 || max_request_bytes == 0 {
        return Err(SubtitleToolkitError::Translation {
            provider: "pipeline",
            message: "translation limits must be greater than zero".into(),
        });
    }

    let mut chunks = Vec::new();
    let mut current = Vec::new();
    let mut current_bytes = 0usize;

    for cue in &document.cues {
        let line = format!("<{}> {}", cue.id, cue.text.replace('\n', "\\N"));
        let line_bytes = line.len();
        if line_bytes > max_request_bytes {
            return Err(SubtitleToolkitError::InvalidTranslation {
                message: format!(
                    "cue <{}> is {} bytes, exceeding provider limit of {} bytes",
                    cue.id, line_bytes, max_request_bytes
                ),
            });
        }

        let separator_bytes = usize::from(!current.is_empty());
        if !current.is_empty()
            && (current.len() >= max_items
                || current_bytes + separator_bytes + line_bytes > max_request_bytes)
        {
            chunks.push(SubtitleDocument {
                cues: std::mem::take(&mut current),
            });
            current_bytes = 0;
        }

        current_bytes += usize::from(!current.is_empty()) + line_bytes;
        current.push(cue.clone());
    }

    if !current.is_empty() {
        chunks.push(SubtitleDocument { cues: current });
    }

    Ok(chunks)
}

pub(crate) fn protect_ass_spans(
    document: &SubtitleDocument,
) -> (SubtitleDocument, ProtectedSpanMap) {
    protect_spans(document, SpanSyntax::Ass)
}

pub(crate) fn protect_markup_spans(
    document: &SubtitleDocument,
) -> (SubtitleDocument, ProtectedSpanMap) {
    protect_spans(document, SpanSyntax::Markup)
}

pub(crate) fn restore_protected_spans(
    document: &mut SubtitleDocument,
    span_map: &ProtectedSpanMap,
) -> Result<()> {
    for cue in &mut document.cues {
        let Some(spans) = span_map.spans.get(&cue.id) else {
            continue;
        };

        let mut previous_position = 0usize;
        for span in spans {
            let positions: Vec<usize> = cue
                .text
                .match_indices(&span.token)
                .map(|(i, _)| i)
                .collect();
            if positions.len() != 1 {
                return Err(SubtitleToolkitError::InvalidTranslation {
                    message: format!(
                        "protected token {} for id <{}> occurred {} times",
                        span.token,
                        cue.id,
                        positions.len()
                    ),
                });
            }
            if positions[0] < previous_position {
                return Err(SubtitleToolkitError::InvalidTranslation {
                    message: format!("protected tokens were reordered for id <{}>", cue.id),
                });
            }
            previous_position = positions[0] + span.token.len();
        }

        for span in spans {
            cue.text = cue.text.replace(&span.token, &span.original);
        }
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum SpanSyntax {
    Ass,
    Markup,
}

fn protect_spans(
    document: &SubtitleDocument,
    syntax: SpanSyntax,
) -> (SubtitleDocument, ProtectedSpanMap) {
    let mut span_map = ProtectedSpanMap::default();
    let cues = document
        .cues
        .iter()
        .map(|cue| {
            let (text, spans) = protect_text_spans(&cue.text, syntax);
            if !spans.is_empty() {
                span_map.spans.insert(cue.id, spans);
            }
            SubtitleCue { id: cue.id, text }
        })
        .collect();
    (SubtitleDocument { cues }, span_map)
}

fn protect_text_spans(text: &str, syntax: SpanSyntax) -> (String, Vec<ProtectedSpan>) {
    let mut output = String::with_capacity(text.len());
    let mut spans = Vec::new();
    let mut cursor = 0usize;

    while cursor < text.len() {
        let remaining = &text[cursor..];
        let start_offset = match syntax {
            SpanSyntax::Ass => remaining.find("{\\"),
            SpanSyntax::Markup => remaining.find('<'),
        };
        let Some(start_offset) = start_offset else {
            output.push_str(remaining);
            break;
        };
        let start = cursor + start_offset;
        output.push_str(&text[cursor..start]);

        let end = match syntax {
            SpanSyntax::Ass => text[start..].find('}').map(|offset| start + offset + 1),
            SpanSyntax::Markup => text[start..].find('>').map(|offset| start + offset + 1),
        };
        let Some(end) = end else {
            output.push_str(&text[start..]);
            break;
        };

        let original = &text[start..end];
        if matches!(syntax, SpanSyntax::Markup) && !looks_like_markup_tag(original) {
            output.push('<');
            cursor = start + 1;
            continue;
        }

        let token = format!("{PROTECTED_TAG_PREFIX}{}]]", spans.len());
        output.push_str(&token);
        spans.push(ProtectedSpan {
            token,
            original: original.to_string(),
        });
        cursor = end;
    }

    (output, spans)
}

fn looks_like_markup_tag(value: &str) -> bool {
    value
        .strip_prefix('<')
        .and_then(|value| value.strip_suffix('>'))
        .and_then(|value| value.trim_start().chars().next())
        .is_some_and(|character| {
            character == '/' || character.is_ascii_alphabetic() || character.is_ascii_digit()
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::subtitles::model::{SubtitleCue, SubtitleDocument};

    #[test]
    fn formats_numbered_text() {
        let document = SubtitleDocument {
            cues: vec![
                SubtitleCue {
                    id: 1,
                    text: "hello".into(),
                },
                SubtitleCue {
                    id: 2,
                    text: "world".into(),
                },
            ],
        };

        assert_eq!(to_numbered_text(&document), "<1> hello\n<2> world");
    }

    #[test]
    fn parses_multiline_numbered_text() {
        let parsed = parse_numbered_text("<1> olá\ncontinua\n<2> mundo", &[1, 2]).unwrap();

        assert_eq!(parsed.get(&1).unwrap(), "olá\ncontinua");
        assert_eq!(parsed.get(&2).unwrap(), "mundo");
    }

    #[test]
    fn decodes_serialized_hard_breaks() {
        let parsed = parse_numbered_text(r"<1> linha um\Nlinha dois", &[1]).unwrap();
        assert_eq!(parsed.get(&1).unwrap(), "linha um\nlinha dois");
    }

    #[test]
    fn rejects_empty_translation_values() {
        let error = parse_numbered_text("<1>   ", &[1]).unwrap_err();
        assert!(error.to_string().contains("empty translation"));
    }

    #[test]
    fn rejects_missing_ids() {
        let error = parse_numbered_text("<1> olá", &[1, 2]).unwrap_err();

        assert!(error.to_string().contains("missing id <2>"));
    }

    #[test]
    fn strip_tags_extracts_leading_tags() {
        let (clean, tags) = strip_override_tags(r"{\pos(857.6,122.4)}{\an7}STATUS");
        assert_eq!(clean, "STATUS");
        assert_eq!(tags, r"{\pos(857.6,122.4)}{\an7}");
    }

    #[test]
    fn strip_tags_no_tags() {
        let (clean, tags) = strip_override_tags("Hello world");
        assert_eq!(clean, "Hello world");
        assert!(tags.is_empty());
    }

    #[test]
    fn strip_tags_only_tags() {
        let (clean, tags) = strip_override_tags(r"{\pos(1,2)}{\an7}");
        assert!(clean.is_empty());
        assert_eq!(tags, r"{\pos(1,2)}{\an7}");
    }

    #[test]
    fn strip_tags_single_tag() {
        let (clean, tags) = strip_override_tags(r"{\b1}Bold text");
        assert_eq!(clean, "Bold text");
        assert_eq!(tags, r"{\b1}");
    }

    #[test]
    fn strip_tags_inner_braces_not_confused() {
        let (clean, tags) = strip_override_tags(r"{\pos(1.0,2.0)}Hello");
        assert_eq!(clean, "Hello");
        assert_eq!(tags, r"{\pos(1.0,2.0)}");
    }

    #[test]
    fn reinject_tags_roundtrip() {
        let document = SubtitleDocument {
            cues: vec![
                SubtitleCue {
                    id: 1,
                    text: r"{\pos(1,2)}{\an7}STATUS".into(),
                },
                SubtitleCue {
                    id: 2,
                    text: "No tags here".into(),
                },
            ],
        };

        let (clean_doc, tag_map) = strip_tags(&document);
        assert_eq!(clean_doc.cues[0].text, "STATUS");
        assert_eq!(clean_doc.cues[1].text, "No tags here");

        let mut restored = clean_doc;
        reinject_tags(&mut restored, &tag_map);
        assert_eq!(restored.cues[0].text, r"{\pos(1,2)}{\an7}STATUS");
        assert_eq!(restored.cues[1].text, "No tags here");
    }

    #[test]
    fn reinject_tags_empty() {
        let mut doc = SubtitleDocument {
            cues: vec![SubtitleCue {
                id: 1,
                text: "clean".into(),
            }],
        };
        let tag_map = HashMap::new();
        reinject_tags(&mut doc, &tag_map);
        assert_eq!(doc.cues[0].text, "clean");
    }

    #[test]
    fn to_numbered_text_after_stripping() {
        let document = SubtitleDocument {
            cues: vec![
                SubtitleCue {
                    id: 1,
                    text: r"{\pos(1,2)}{\an7}STATUS".into(),
                },
                SubtitleCue {
                    id: 2,
                    text: "Hello".into(),
                },
            ],
        };

        let (clean, _) = strip_tags(&document);
        let numbered = to_numbered_text(&clean);
        assert_eq!(numbered, "<1> STATUS\n<2> Hello");
    }

    #[test]
    fn chunk_document_splits_when_needed() {
        let document = SubtitleDocument {
            cues: (1..=10)
                .map(|i| SubtitleCue {
                    id: i,
                    text: format!("line {i}"),
                })
                .collect(),
        };

        // Each line is ~11 chars ("<N> line N"), total ~120 chars
        // With limit 50, should split into multiple chunks
        let chunks = chunk_document(&document, 50);
        assert!(chunks.len() > 1);

        // All cues preserved across chunks
        let total_cues: usize = chunks.iter().map(|c| c.cues.len()).sum();
        assert_eq!(total_cues, 10);

        // IDs preserved
        let all_ids: Vec<usize> = chunks
            .iter()
            .flat_map(|c| c.cues.iter().map(|cue| cue.id))
            .collect();
        assert_eq!(all_ids, (1..=10).collect::<Vec<_>>());
    }

    #[test]
    fn chunk_document_single_chunk_when_fits() {
        let document = SubtitleDocument {
            cues: vec![
                SubtitleCue {
                    id: 1,
                    text: "hello".into(),
                },
                SubtitleCue {
                    id: 2,
                    text: "world".into(),
                },
            ],
        };

        let chunks = chunk_document(&document, 10000);
        assert_eq!(chunks.len(), 1);
        assert_eq!(chunks[0].cues.len(), 2);
    }

    #[test]
    fn chunk_document_oversized_cue_gets_own_chunk() {
        let big_text = "x".repeat(200);
        let document = SubtitleDocument {
            cues: vec![
                SubtitleCue {
                    id: 1,
                    text: big_text.clone(),
                },
                SubtitleCue {
                    id: 2,
                    text: "small".into(),
                },
            ],
        };

        let chunks = chunk_document(&document, 50);
        assert_eq!(chunks.len(), 2);
        assert_eq!(chunks[0].cues[0].id, 1);
        assert_eq!(chunks[0].cues[0].text, big_text);
        assert_eq!(chunks[1].cues[0].id, 2);
    }

    #[test]
    fn chunk_document_empty() {
        let document = SubtitleDocument { cues: vec![] };
        let chunks = chunk_document(&document, 5000);
        assert!(chunks.is_empty());
    }

    #[test]
    fn chunk_document_with_limits_respects_items_and_bytes() {
        let document = SubtitleDocument {
            cues: (1..=5)
                .map(|id| SubtitleCue {
                    id,
                    text: "abcdefghij".into(),
                })
                .collect(),
        };

        let chunks = chunk_document_with_limits(&document, 2, 31).unwrap();
        assert_eq!(
            chunks
                .iter()
                .map(|chunk| chunk.cues.len())
                .collect::<Vec<_>>(),
            vec![2, 2, 1]
        );
        assert!(
            chunks
                .iter()
                .all(|chunk| to_numbered_text(chunk).len() <= 31)
        );
    }

    #[test]
    fn protects_and_restores_inline_ass_tags() {
        let document = SubtitleDocument {
            cues: vec![SubtitleCue {
                id: 1,
                text: r"Hello {\i1}world{\i0}!".into(),
            }],
        };
        let (mut protected, spans) = protect_ass_spans(&document);
        assert_eq!(
            protected.cues[0].text,
            "Hello [[PSY_TAG_0]]world[[PSY_TAG_1]]!"
        );
        protected.cues[0].text = "OlÃ¡ [[PSY_TAG_0]]mundo[[PSY_TAG_1]]!".into();
        restore_protected_spans(&mut protected, &spans).unwrap();
        assert_eq!(protected.cues[0].text, r"OlÃ¡ {\i1}mundo{\i0}!");
    }

    #[test]
    fn rejects_missing_protected_tag() {
        let document = SubtitleDocument {
            cues: vec![SubtitleCue {
                id: 1,
                text: "<i>Hello</i>".into(),
            }],
        };
        let (mut protected, spans) = protect_markup_spans(&document);
        protected.cues[0].text = "OlÃ¡ [[PSY_TAG_0]]".into();
        let error = restore_protected_spans(&mut protected, &spans).unwrap_err();
        assert!(error.to_string().contains("occurred 0 times"));
    }
}
