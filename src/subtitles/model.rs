/// A parsed subtitle document containing a flat list of cues.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleDocument {
    /// Ordered list of subtitle cues.
    pub cues: Vec<SubtitleCue>,
}

impl SubtitleDocument {
    /// Replace the text of the cue with the given `id`.
    ///
    /// Returns `true` if the cue was found and replaced, `false` if no cue
    /// with that ID exists.
    pub fn replace_text(&mut self, id: usize, text: String) -> bool {
        if let Some(cue) = self.cues.iter_mut().find(|cue| cue.id == id) {
            cue.text = text;
            true
        } else {
            false
        }
    }
}

/// A single subtitle cue with a sequential ID and decoded text.
///
/// IDs are assigned sequentially starting from 1 during ASS parsing.
/// The `text` field contains decoded text (literal `\n` for newlines,
/// no ASS `\N` escapes). Override tags like `{\pos(...)}` are stripped
/// during translation and re-injected afterward.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleCue {
    /// Sequential cue identifier (1-based).
    pub id: usize,
    /// Decoded subtitle text.
    pub text: String,
}
