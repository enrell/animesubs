use std::collections::{BTreeMap, HashMap, HashSet};

use crate::error::{Result, SubtitleToolkitError};

use super::model::{SubtitleCue, SubtitleDocument};

pub fn to_numbered_text(document: &SubtitleDocument) -> String {
    document
        .cues
        .iter()
        .map(|cue| format!("<{}> {}", cue.id, cue.text.replace('\n', "\\N")))
        .collect::<Vec<_>>()
        .join("\n")
}

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

pub fn reinject_tags(document: &mut SubtitleDocument, tag_map: &HashMap<usize, String>) {
    for cue in &mut document.cues {
        if let Some(tags) = tag_map.get(&cue.id) {
            cue.text = format!("{}{}", tags, cue.text);
        }
    }
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
}
