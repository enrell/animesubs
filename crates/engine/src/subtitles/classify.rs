//! ASS cue classification: dialogue, karaoke or song.
//!
//! Ported from shinkai-translator (`src/formats/ass_classifier.rs` and the
//! policy types in `src/domain.rs`, MIT, same author). Karaoke timing tags
//! (`\k`, `\K`) and song markers in `Style`/`Effect`/`Name` are explicit
//! signals; runs of short, unpunctuated, contiguous lines are inferred to be
//! song lyrics. Each kind maps to a [`CueDisposition`] through a
//! [`ClassificationPolicy`], so callers decide whether to translate, keep or
//! flag such cues for review.

use serde::{Deserialize, Serialize};

use super::ass::{AssEvent, AssSubtitle};

/// What a cue is.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CueKind {
    Dialogue,
    Karaoke,
    Song,
}

/// What to do with a cue.
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CueDisposition {
    Translate,
    /// Keep the original text.
    Preserve,
    /// Probably not dialogue; a person should decide.
    Review,
}

impl CueDisposition {
    pub fn is_translatable(self) -> bool {
        matches!(self, Self::Translate)
    }
}

/// How each detected kind is handled and which markers count as songs.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(default)]
pub struct ClassificationPolicy {
    pub karaoke_policy: CueDisposition,
    pub explicit_song_policy: CueDisposition,
    pub inferred_song_policy: CueDisposition,
    pub style_markers: Vec<String>,
    pub effect_markers: Vec<String>,
    pub name_markers: Vec<String>,
    pub enable_inferred_song_detection: bool,
    /// Shortest run of lyric-like cues treated as a song. Must be at least 1.
    pub min_inferred_song_run_length: usize,
}

impl Default for ClassificationPolicy {
    fn default() -> Self {
        let markers: Vec<String> = [
            "karaoke", "kfx", "song", "songs", "lyric", "lyrics", "opening", "ending", "insert",
            "music", "romaji", "op", "ed",
        ]
        .iter()
        .map(|marker| (*marker).to_string())
        .collect();

        Self {
            karaoke_policy: CueDisposition::Preserve,
            explicit_song_policy: CueDisposition::Preserve,
            inferred_song_policy: CueDisposition::Review,
            style_markers: markers.clone(),
            effect_markers: markers.clone(),
            name_markers: markers,
            enable_inferred_song_detection: true,
            min_inferred_song_run_length: 4,
        }
    }
}

/// Classification of one cue.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CueClassification {
    /// Cue ID in the subtitle's [`crate::SubtitleDocument`].
    pub cue_id: usize,
    pub kind: CueKind,
    pub disposition: CueDisposition,
    /// 0-100.
    pub confidence: u8,
    pub reason: Option<String>,
}

/// Counts over a classification.
#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct ClassificationSummary {
    pub total_cues: usize,
    pub translatable_cues: usize,
    pub preserved_cues: usize,
    pub review_cues: usize,
    pub dialogue_cues: usize,
    pub karaoke_cues: usize,
    pub song_cues: usize,
}

/// Classify every dialogue event of an ASS subtitle, in file order.
pub fn classify_ass(ass: &AssSubtitle, policy: &ClassificationPolicy) -> Vec<CueClassification> {
    let events = ass.events();
    let mut classified: Vec<CueClassification> = events
        .iter()
        .map(|event| {
            classify_explicit(event, policy).unwrap_or(CueClassification {
                cue_id: event.id(),
                kind: CueKind::Dialogue,
                disposition: CueDisposition::Translate,
                confidence: 0,
                reason: None,
            })
        })
        .collect();

    if policy.enable_inferred_song_detection {
        classify_inferred_song_runs(&events, &mut classified, policy);
    }
    classified
}

/// Totals for a classification, e.g. for a report.
pub fn summarize(classified: &[CueClassification]) -> ClassificationSummary {
    let mut summary = ClassificationSummary {
        total_cues: classified.len(),
        ..ClassificationSummary::default()
    };
    for cue in classified {
        match cue.disposition {
            CueDisposition::Translate => summary.translatable_cues += 1,
            CueDisposition::Preserve => summary.preserved_cues += 1,
            CueDisposition::Review => summary.review_cues += 1,
        }
        match cue.kind {
            CueKind::Dialogue => summary.dialogue_cues += 1,
            CueKind::Karaoke => summary.karaoke_cues += 1,
            CueKind::Song => summary.song_cues += 1,
        }
    }
    summary
}

fn classify_explicit(
    event: &AssEvent<'_>,
    policy: &ClassificationPolicy,
) -> Option<CueClassification> {
    if contains_karaoke_override(event.raw_text()) {
        return Some(CueClassification {
            cue_id: event.id(),
            kind: CueKind::Karaoke,
            disposition: policy.karaoke_policy,
            confidence: 100,
            reason: Some("karaoke timing override detected".into()),
        });
    }

    metadata_song_reason(event, policy).map(|reason| CueClassification {
        cue_id: event.id(),
        kind: CueKind::Song,
        disposition: policy.explicit_song_policy,
        confidence: 95,
        reason: Some(reason),
    })
}

fn classify_inferred_song_runs(
    events: &[AssEvent<'_>],
    classified: &mut [CueClassification],
    policy: &ClassificationPolicy,
) {
    // Decided up front: the loop only rewrites cues it has already passed.
    let candidate: Vec<bool> = events
        .iter()
        .zip(classified.iter())
        .map(|(event, cue)| cue.kind == CueKind::Dialogue && looks_like_song_candidate(event))
        .collect();
    let mut index = 0usize;
    while index < events.len() {
        if !candidate[index] {
            index += 1;
            continue;
        }

        let run_start = index;
        let mut run_end = index;
        while run_end + 1 < events.len()
            && candidate[run_end + 1]
            && events_are_contiguous(&events[run_end], &events[run_end + 1])
        {
            run_end += 1;
        }

        let run_length = run_end - run_start + 1;
        if run_length >= policy.min_inferred_song_run_length.max(1) {
            let reason = format!("inferred song run of {run_length} short contiguous cues");
            for cue in &mut classified[run_start..=run_end] {
                cue.kind = CueKind::Song;
                cue.disposition = policy.inferred_song_policy;
                cue.confidence = 60;
                cue.reason = Some(reason.clone());
            }
        }
        index = run_end + 1;
    }
}

fn contains_karaoke_override(text: &str) -> bool {
    text.as_bytes()
        .windows(2)
        .any(|window| window[0] == b'\\' && matches!(window[1], b'k' | b'K'))
}

fn metadata_song_reason(event: &AssEvent<'_>, policy: &ClassificationPolicy) -> Option<String> {
    match_marker_field("Style", event, &policy.style_markers)
        .or_else(|| match_marker_field("Effect", event, &policy.effect_markers))
        .or_else(|| match_marker_field("Name", event, &policy.name_markers))
}

fn match_marker_field(field: &str, event: &AssEvent<'_>, markers: &[String]) -> Option<String> {
    let value = event.field(field)?;
    let normalized_value = normalize_words(value);

    for marker in markers {
        let normalized_marker = normalize_words(marker);
        if normalized_marker.is_empty() {
            continue;
        }
        // "op"/"ed" only match as whole tokens (OP, OP2, ED1), not inside words.
        if normalized_marker == "op" || normalized_marker == "ed" {
            if tokenize_ascii_words(value)
                .iter()
                .any(|token| song_marker_token(token, &normalized_marker))
            {
                return Some(format!("song marker `{marker}` matched {field}"));
            }
            continue;
        }
        if normalized_value.contains(&normalized_marker) {
            return Some(format!("song marker `{marker}` matched {field}"));
        }
    }
    None
}

/// Lowercase ASCII alphanumerics, other runs collapsed to one space.
fn normalize_words(value: &str) -> String {
    let mut normalized = String::new();
    let mut previous_was_space = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            normalized.push(character.to_ascii_lowercase());
            previous_was_space = false;
        } else if !previous_was_space && !normalized.is_empty() {
            normalized.push(' ');
            previous_was_space = true;
        }
    }
    normalized.trim().to_string()
}

fn tokenize_ascii_words(value: &str) -> Vec<String> {
    value
        .split(|character: char| !character.is_ascii_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(str::to_ascii_lowercase)
        .collect()
}

fn song_marker_token(token: &str, prefix: &str) -> bool {
    token
        .strip_prefix(prefix)
        .is_some_and(|rest| rest.chars().all(|character| character.is_ascii_digit()))
}

fn looks_like_song_candidate(event: &AssEvent<'_>) -> bool {
    let visible = visible_text(event.raw_text());
    let compact = visible.trim();
    if compact.is_empty() {
        return false;
    }

    let word_count = compact.split_whitespace().count();
    let visible_chars = compact.chars().filter(|c| !c.is_whitespace()).count();
    let has_letters = compact.chars().any(char::is_alphabetic);
    let has_sentence_punctuation = compact
        .chars()
        .any(|c| matches!(c, '.' | '!' | '?' | ';' | ':' | '"'));

    has_letters && word_count <= 8 && visible_chars <= 64 && !has_sentence_punctuation
}

/// Text without `{...}` override blocks; `\N`, `\n`, `\h` become spaces.
fn visible_text(text: &str) -> String {
    let mut visible = String::new();
    let mut in_override = false;
    let mut characters = text.chars();
    while let Some(character) = characters.next() {
        if in_override {
            if character == '}' {
                in_override = false;
            }
            continue;
        }
        match character {
            '{' => in_override = true,
            '\\' => match characters.next() {
                Some('N') | Some('n') | Some('h') | None => visible.push(' '),
                Some(next) => {
                    visible.push(' ');
                    visible.push(next);
                }
            },
            _ => visible.push(character),
        }
    }
    visible
}

fn events_are_contiguous(previous: &AssEvent<'_>, next: &AssEvent<'_>) -> bool {
    match (
        previous.end().and_then(parse_ass_timestamp),
        next.start().and_then(parse_ass_timestamp),
    ) {
        // Up to 2.5 s between lines still counts as one song.
        (Some(previous_end), Some(next_start)) => next_start.saturating_sub(previous_end) <= 250,
        _ => true,
    }
}

/// `H:MM:SS.cc` in centiseconds.
fn parse_ass_timestamp(value: &str) -> Option<u64> {
    let mut parts = value.trim().split(':');
    let hours = parts.next()?.parse::<u64>().ok()?;
    let minutes = parts.next()?.parse::<u64>().ok()?;
    let seconds_and_centis = parts.next()?;
    if parts.next().is_some() {
        return None;
    }
    let (seconds, centis) = seconds_and_centis.split_once('.')?;
    let seconds = seconds.parse::<u64>().ok()?;
    let centis = centis.parse::<u64>().ok()?;
    Some(hours * 360_000 + minutes * 6_000 + seconds * 100 + centis)
}

#[cfg(test)]
mod tests {
    use super::*;

    const HEADER: &str = "[Events]\n\
        Format: Layer, Start, End, Style, Name, MarginL, MarginR, MarginV, Effect, Text\n";

    fn parse(events: &str) -> AssSubtitle {
        AssSubtitle::parse(&format!("{HEADER}{events}")).unwrap()
    }

    #[test]
    fn classifies_karaoke_and_song_metadata_with_policy() {
        let ass = parse(
            "Dialogue: 0,0:00:01.00,0:00:03.00,Default,,0,0,0,,Hello there!\n\
             Dialogue: 0,0:00:03.20,0:00:04.20,Default,,0,0,0,,{\\k20}Ka{\\k20}ra\n\
             Dialogue: 0,0:00:05.00,0:00:06.00,Lyrics,,0,0,0,,Shining star\n",
        );
        let classified = classify_ass(&ass, &ClassificationPolicy::default());

        assert_eq!(classified[0].kind, CueKind::Dialogue);
        assert_eq!(classified[0].disposition, CueDisposition::Translate);
        assert_eq!(classified[1].kind, CueKind::Karaoke);
        assert_eq!(classified[1].disposition, CueDisposition::Preserve);
        assert_eq!(classified[2].kind, CueKind::Song);
        assert_eq!(classified[2].disposition, CueDisposition::Preserve);
        let summary = summarize(&classified);
        assert_eq!((summary.karaoke_cues, summary.song_cues), (1, 1));
        assert_eq!(
            classified.iter().map(|c| c.cue_id).collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn detects_song_runs_without_explicit_karaoke_tags() {
        let ass = parse(
            "Dialogue: 0,0:00:01.00,0:00:03.00,Q0,,0,0,0,,Muteki no egao\n\
             Dialogue: 0,0:00:03.10,0:00:05.00,Q0,,0,0,0,,Shiritai sono himitsu\n\
             Dialogue: 0,0:00:05.10,0:00:07.00,Q0,,0,0,0,,Nuketeru toko sae\n\
             Dialogue: 0,0:00:07.10,0:00:09.00,Q0,,0,0,0,,Kanpeki de usotsuki na kimi\n\
             Dialogue: 0,0:00:12.00,0:00:14.00,Default,,0,0,0,,What happened here?\n",
        );
        let classified = classify_ass(&ass, &ClassificationPolicy::default());

        for cue in &classified[..4] {
            assert_eq!(cue.kind, CueKind::Song);
            assert_eq!(cue.disposition, CueDisposition::Review);
        }
        assert_eq!(classified[4].kind, CueKind::Dialogue);
        assert_eq!(summarize(&classified).review_cues, 4);
    }

    #[test]
    fn can_override_inferred_song_policy_to_translate() {
        let ass = parse(
            "Dialogue: 0,0:00:01.00,0:00:03.00,Q0,,0,0,0,,Muteki no egao\n\
             Dialogue: 0,0:00:03.10,0:00:05.00,Q0,,0,0,0,,Shiritai sono himitsu\n\
             Dialogue: 0,0:00:05.10,0:00:07.00,Q0,,0,0,0,,Nuketeru toko sae\n\
             Dialogue: 0,0:00:07.10,0:00:09.00,Q0,,0,0,0,,Kanpeki de usotsuki na kimi\n",
        );
        let policy = ClassificationPolicy {
            inferred_song_policy: CueDisposition::Translate,
            ..ClassificationPolicy::default()
        };
        let classified = classify_ass(&ass, &policy);

        assert!(classified.iter().all(|c| c.disposition.is_translatable()));
        let summary = summarize(&classified);
        assert_eq!((summary.translatable_cues, summary.review_cues), (4, 0));
    }

    #[test]
    fn op_and_ed_markers_only_match_whole_tokens() {
        let ass = parse(
            "Dialogue: 0,0:00:01.00,0:00:02.00,OP2,,0,0,0,,Sora e\n\
             Dialogue: 0,0:00:10.00,0:00:12.00,Edgar,,0,0,0,,I am not a song, I promise.\n\
             Dialogue: 0,0:00:20.00,0:00:22.00,Default,Opera Singer,0,0,0,,Nor am I, really.\n",
        );
        let classified = classify_ass(&ass, &ClassificationPolicy::default());
        assert_eq!(classified[0].kind, CueKind::Song, "OP2 is an opening style");
        assert_eq!(classified[1].kind, CueKind::Dialogue, "`Edgar` is not `ED`");
        assert_eq!(classified[2].kind, CueKind::Dialogue, "`Opera` is not `OP`");
    }

    #[test]
    fn gaps_break_inferred_runs() {
        // Four lyric-like lines, but a 5 s gap splits them into runs of 2.
        let ass = parse(
            "Dialogue: 0,0:00:01.00,0:00:02.00,Q0,,0,0,0,,Muteki no egao\n\
             Dialogue: 0,0:00:02.10,0:00:03.00,Q0,,0,0,0,,Shiritai sono himitsu\n\
             Dialogue: 0,0:00:08.00,0:00:09.00,Q0,,0,0,0,,Nuketeru toko sae\n\
             Dialogue: 0,0:00:09.10,0:00:10.00,Q0,,0,0,0,,Kanpeki de usotsuki\n",
        );
        let classified = classify_ass(&ass, &ClassificationPolicy::default());
        assert!(classified.iter().all(|c| c.kind == CueKind::Dialogue));
    }
}
