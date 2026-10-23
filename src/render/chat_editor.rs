//! Unicode-aware chat input, selection, history and Brigadier UTF-16 ranges.
use std::collections::VecDeque;
#[derive(Clone, Default)]
pub struct Editor {
    pub text: String,
    pub cursor: usize,
    pub anchor: usize,
    pub history: VecDeque<String>,
    history_position: usize,
    draft: String,
}
impl Editor {
    pub fn open(&mut self, initial: &str) {
        self.text = initial.into();
        self.cursor = self.text.len();
        self.anchor = self.cursor;
        self.history_position = self.history.len();
        self.draft = self.text.clone();
    }
    pub fn selected(&self) -> std::ops::Range<usize> {
        self.cursor.min(self.anchor)..self.cursor.max(self.anchor)
    }
    pub fn insert(&mut self, value: &str) {
        self.insert_limited(value, 256);
    }
    pub fn insert_limited(&mut self, value: &str, limit: usize) {
        let selection = self.selected();
        let available = limit.saturating_sub(
            self.text[..selection.start].encode_utf16().count()
                + self.text[selection.end..].encode_utf16().count(),
        );
        let mut used = 0;
        let filtered: String = value
            .chars()
            .filter(|&c| c >= ' ' && c != '\u{7f}' && c != '§')
            .take_while(|c| {
                used += c.len_utf16();
                used <= available
            })
            .collect();
        self.text.replace_range(selection.clone(), &filtered);
        self.cursor = selection.start + filtered.len();
        self.anchor = self.cursor;
    }
    pub fn move_to(&mut self, position: usize, select: bool) {
        self.cursor = position.min(self.text.len());
        if !select {
            self.anchor = self.cursor;
        }
    }
    pub fn horizontal(&mut self, right: bool, word: bool, select: bool) {
        let mut p = self.cursor;
        if right {
            if word {
                while p < self.text.len() && !self.text[p..].starts_with(' ') {
                    p += self.text[p..].chars().next().unwrap().len_utf8();
                }
                while p < self.text.len() && self.text[p..].starts_with(' ') {
                    p += 1;
                }
            } else if let Some(c) = self.text[p..].chars().next() {
                p += c.len_utf8();
            }
        } else if word {
            while p > 0 && self.text[..p].ends_with(' ') {
                p -= 1;
            }
            while p > 0 && !self.text[..p].ends_with(' ') {
                p -= self.text[..p].chars().next_back().unwrap().len_utf8();
            }
        } else if let Some(c) = self.text[..p].chars().next_back() {
            p -= c.len_utf8();
        }
        self.move_to(p, select);
    }
    pub fn delete(&mut self, right: bool, word: bool) {
        if self.cursor == self.anchor {
            self.horizontal(right, word, true);
        }
        let range = self.selected();
        self.text.replace_range(range.clone(), "");
        self.cursor = range.start;
        self.anchor = self.cursor;
    }
    pub fn history(&mut self, older: bool) {
        if self.history_position == self.history.len() {
            self.draft = self.text.clone();
        }
        self.history_position = if older {
            self.history_position.saturating_sub(1)
        } else {
            (self.history_position + 1).min(self.history.len())
        };
        self.text = if self.history_position == self.history.len() {
            self.draft.clone()
        } else {
            self.history[self.history_position].clone()
        };
        self.cursor = self.text.len();
        self.anchor = self.cursor;
    }
    pub fn submit(&mut self) -> Option<String> {
        let text = self.text.split_whitespace().collect::<Vec<_>>().join(" ");
        if text.is_empty() {
            return None;
        }
        if self.history.back() != Some(&text) {
            if self.history.len() == 100 {
                self.history.pop_front();
            }
            self.history.push_back(text.clone());
        }
        Some(text)
    }
    pub fn replace_utf16(&mut self, start: usize, length: usize, value: &str) -> bool {
        let byte = |target| {
            let mut units = 0;
            for (i, c) in self.text.char_indices() {
                if units == target {
                    return Some(i);
                }
                units += c.len_utf16();
            }
            if units == target {
                Some(self.text.len())
            } else {
                None
            }
        };
        let (Some(a), Some(b)) = (byte(start), start.checked_add(length).and_then(byte)) else {
            return false;
        };
        self.cursor = a;
        self.anchor = b;
        self.insert(value);
        true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unicode_selection_and_suggestion_ranges_do_not_split_surrogates() {
        let mut e = Editor::default();
        e.open("/msg 😀 Al");
        assert!(e.replace_utf16(8, 2, "Alice"));
        assert_eq!(e.text, "/msg 😀 Alice");
        assert!(!e.replace_utf16(6, 1, "x"));
        e.horizontal(false, true, true);
        e.insert("Боб");
        assert_eq!(e.text, "/msg 😀 Боб");
        e.delete(false, false);
        assert_eq!(e.text, "/msg 😀 Бо");
        e.open("");
        e.insert(&"😀".repeat(200));
        assert_eq!(e.text.chars().count(), 128);
        e.anchor = 0;
        e.insert("new\n§ text");
        assert_eq!(e.text, "new text");
    }
    #[test]
    fn chat_history_restores_draft_and_deduplicates_consecutive_messages() {
        let mut e = Editor::default();
        e.open(" hello  world ");
        assert_eq!(e.submit().as_deref(), Some("hello world"));
        e.submit();
        assert_eq!(e.history.len(), 1);
        e.open("draft");
        e.history(true);
        assert_eq!(e.text, "hello world");
        e.history(false);
        assert_eq!(e.text, "draft");
    }
}
