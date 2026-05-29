use std::collections::{BTreeMap, HashSet};

use crate::error::{Result, SubtitleToolkitError};

use super::model::SubtitleDocument;

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
}
