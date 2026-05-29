#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleDocument {
    pub cues: Vec<SubtitleCue>,
}

impl SubtitleDocument {
    pub fn replace_text(&mut self, id: usize, text: String) -> bool {
        if let Some(cue) = self.cues.iter_mut().find(|cue| cue.id == id) {
            cue.text = text;
            true
        } else {
            false
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SubtitleCue {
    pub id: usize,
    pub text: String,
}
