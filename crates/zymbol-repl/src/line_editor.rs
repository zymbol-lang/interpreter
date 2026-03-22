//! Line editor with history, selection, and clipboard support

use arboard::Clipboard;
use std::collections::VecDeque;

/// Maximum number of commands to keep in history
const MAX_HISTORY: usize = 100;

/// Line editor for the REPL
pub struct LineEditor {
    /// Current line being edited
    current_buffer: String,
    /// Cursor position (byte offset in the string)
    cursor_pos: usize,
    /// Command history (most recent at front)
    history: VecDeque<String>,
    /// Current position in history navigation (-1 = current buffer)
    history_index: isize,
    /// Saved current buffer when navigating history
    saved_buffer: String,
    /// Selection start position (byte offset)
    selection_start: Option<usize>,
    /// Selection end position (byte offset)
    selection_end: Option<usize>,
    /// System clipboard
    clipboard: Option<Clipboard>,
}

impl Default for LineEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl LineEditor {
    /// Create a new line editor
    pub fn new() -> Self {
        let clipboard = Clipboard::new().ok();
        Self {
            current_buffer: String::new(),
            cursor_pos: 0,
            history: VecDeque::new(),
            history_index: -1,
            saved_buffer: String::new(),
            selection_start: None,
            selection_end: None,
            clipboard,
        }
    }

    /// Get the current buffer content
    pub fn buffer(&self) -> &str {
        &self.current_buffer
    }

    /// Get the cursor position
    pub fn cursor_pos(&self) -> usize {
        self.cursor_pos
    }

    /// Get selection range if any (start, end) where start <= end
    pub fn selection(&self) -> Option<(usize, usize)> {
        match (self.selection_start, self.selection_end) {
            (Some(start), Some(end)) => {
                if start <= end {
                    Some((start, end))
                } else {
                    Some((end, start))
                }
            }
            _ => None,
        }
    }

    /// Check if there's an active selection
    pub fn has_selection(&self) -> bool {
        self.selection().is_some()
    }

    /// Clear the current buffer and cursor
    pub fn clear(&mut self) {
        self.current_buffer.clear();
        self.cursor_pos = 0;
        self.clear_selection();
    }

    /// Set the buffer content (used for restoring from history)
    #[allow(dead_code)]
    pub fn set_buffer(&mut self, content: String) {
        self.current_buffer = content;
        self.cursor_pos = self.current_buffer.len();
        self.clear_selection();
    }

    /// Insert a character at the cursor position
    pub fn insert_char(&mut self, c: char) {
        // Delete selection first if any
        if self.has_selection() {
            self.delete_selection();
        }

        self.current_buffer.insert(self.cursor_pos, c);
        self.cursor_pos += c.len_utf8();
    }

    /// Insert a string at the cursor position
    pub fn insert_str(&mut self, s: &str) {
        // Delete selection first if any
        if self.has_selection() {
            self.delete_selection();
        }

        self.current_buffer.insert_str(self.cursor_pos, s);
        self.cursor_pos += s.len();
    }

    /// Delete character before cursor (backspace)
    pub fn backspace(&mut self) -> bool {
        // Delete selection first if any
        if self.has_selection() {
            self.delete_selection();
            return true;
        }

        if self.cursor_pos > 0 {
            // Find the previous character boundary
            let prev_pos = self.prev_char_boundary();
            self.current_buffer.drain(prev_pos..self.cursor_pos);
            self.cursor_pos = prev_pos;
            true
        } else {
            false
        }
    }

    /// Delete character at cursor (delete key)
    pub fn delete(&mut self) -> bool {
        // Delete selection first if any
        if self.has_selection() {
            self.delete_selection();
            return true;
        }

        if self.cursor_pos < self.current_buffer.len() {
            let next_pos = self.next_char_boundary();
            self.current_buffer.drain(self.cursor_pos..next_pos);
            true
        } else {
            false
        }
    }

    /// Move cursor left by one character
    pub fn cursor_left(&mut self) {
        self.clear_selection();
        if self.cursor_pos > 0 {
            self.cursor_pos = self.prev_char_boundary();
        }
    }

    /// Move cursor right by one character
    pub fn cursor_right(&mut self) {
        self.clear_selection();
        if self.cursor_pos < self.current_buffer.len() {
            self.cursor_pos = self.next_char_boundary();
        }
    }

    /// Move cursor to start of line
    pub fn cursor_home(&mut self) {
        self.clear_selection();
        self.cursor_pos = 0;
    }

    /// Move cursor to end of line
    pub fn cursor_end(&mut self) {
        self.clear_selection();
        self.cursor_pos = self.current_buffer.len();
    }

    /// Move cursor left and extend selection
    pub fn select_left(&mut self) {
        if self.cursor_pos > 0 {
            self.start_or_extend_selection();
            self.cursor_pos = self.prev_char_boundary();
            self.selection_end = Some(self.cursor_pos);
        }
    }

    /// Move cursor right and extend selection
    pub fn select_right(&mut self) {
        if self.cursor_pos < self.current_buffer.len() {
            self.start_or_extend_selection();
            self.cursor_pos = self.next_char_boundary();
            self.selection_end = Some(self.cursor_pos);
        }
    }

    /// Select to start of line
    pub fn select_home(&mut self) {
        self.start_or_extend_selection();
        self.cursor_pos = 0;
        self.selection_end = Some(0);
    }

    /// Select to end of line
    pub fn select_end(&mut self) {
        self.start_or_extend_selection();
        self.cursor_pos = self.current_buffer.len();
        self.selection_end = Some(self.cursor_pos);
    }

    /// Navigate to previous command in history
    pub fn history_up(&mut self) -> bool {
        if self.history.is_empty() {
            return false;
        }

        // Save current buffer when starting history navigation
        if self.history_index == -1 {
            self.saved_buffer = self.current_buffer.clone();
        }

        let max_index = self.history.len() as isize - 1;
        if self.history_index < max_index {
            self.history_index += 1;
            self.current_buffer = self.history[self.history_index as usize].clone();
            self.cursor_pos = self.current_buffer.len();
            self.clear_selection();
            true
        } else {
            false
        }
    }

    /// Navigate to next command in history
    pub fn history_down(&mut self) -> bool {
        if self.history_index > -1 {
            self.history_index -= 1;
            if self.history_index == -1 {
                // Restore saved buffer
                self.current_buffer = self.saved_buffer.clone();
            } else {
                self.current_buffer = self.history[self.history_index as usize].clone();
            }
            self.cursor_pos = self.current_buffer.len();
            self.clear_selection();
            true
        } else {
            false
        }
    }

    /// Add a command to history
    pub fn add_to_history(&mut self, command: String) {
        // Don't add empty commands or duplicates of the last command
        if command.is_empty() {
            return;
        }
        if let Some(last) = self.history.front() {
            if last == &command {
                return;
            }
        }

        self.history.push_front(command);
        if self.history.len() > MAX_HISTORY {
            self.history.pop_back();
        }

        // Reset history navigation
        self.history_index = -1;
        self.saved_buffer.clear();
    }

    /// Get history as a vector of strings (for HISTORY command)
    pub fn get_history(&self) -> Vec<&str> {
        self.history.iter().map(|s| s.as_str()).collect()
    }

    /// Copy selected text to clipboard
    pub fn copy_selection(&mut self) -> bool {
        if let Some((start, end)) = self.selection() {
            if let Some(ref mut clipboard) = self.clipboard {
                let selected_text = &self.current_buffer[start..end];
                if clipboard.set_text(selected_text.to_string()).is_ok() {
                    return true;
                }
            }
        }
        false
    }

    /// Cut selected text to clipboard
    pub fn cut_selection(&mut self) -> bool {
        if self.copy_selection() {
            self.delete_selection();
            true
        } else {
            false
        }
    }

    /// Paste from clipboard
    pub fn paste(&mut self) -> bool {
        if let Some(ref mut clipboard) = self.clipboard {
            if let Ok(text) = clipboard.get_text() {
                // Remove newlines from pasted text (single-line REPL)
                let text = text.replace('\n', " ").replace('\r', "");
                self.insert_str(&text);
                return true;
            }
        }
        false
    }

    /// Submit the current line (returns the buffer content and clears it)
    pub fn submit(&mut self) -> String {
        let line = std::mem::take(&mut self.current_buffer);
        self.cursor_pos = 0;
        self.clear_selection();
        self.history_index = -1;
        self.saved_buffer.clear();
        line
    }

    // === Private helpers ===

    /// Find the byte position of the previous character
    fn prev_char_boundary(&self) -> usize {
        let mut pos = self.cursor_pos;
        if pos > 0 {
            pos -= 1;
            while pos > 0 && !self.current_buffer.is_char_boundary(pos) {
                pos -= 1;
            }
        }
        pos
    }

    /// Find the byte position of the next character
    fn next_char_boundary(&self) -> usize {
        let mut pos = self.cursor_pos;
        if pos < self.current_buffer.len() {
            pos += 1;
            while pos < self.current_buffer.len() && !self.current_buffer.is_char_boundary(pos) {
                pos += 1;
            }
        }
        pos
    }

    /// Start a new selection or extend existing one
    fn start_or_extend_selection(&mut self) {
        if self.selection_start.is_none() {
            self.selection_start = Some(self.cursor_pos);
            self.selection_end = Some(self.cursor_pos);
        }
    }

    /// Clear the current selection
    fn clear_selection(&mut self) {
        self.selection_start = None;
        self.selection_end = None;
    }

    /// Delete the selected text
    fn delete_selection(&mut self) {
        if let Some((start, end)) = self.selection() {
            self.current_buffer.drain(start..end);
            self.cursor_pos = start;
            self.clear_selection();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_insert_and_cursor() {
        let mut editor = LineEditor::new();
        editor.insert_char('a');
        editor.insert_char('b');
        editor.insert_char('c');
        assert_eq!(editor.buffer(), "abc");
        assert_eq!(editor.cursor_pos(), 3);
    }

    #[test]
    fn test_backspace() {
        let mut editor = LineEditor::new();
        editor.insert_str("hello");
        assert!(editor.backspace());
        assert_eq!(editor.buffer(), "hell");
        assert_eq!(editor.cursor_pos(), 4);
    }

    #[test]
    fn test_cursor_movement() {
        let mut editor = LineEditor::new();
        editor.insert_str("hello");
        editor.cursor_home();
        assert_eq!(editor.cursor_pos(), 0);
        editor.cursor_right();
        assert_eq!(editor.cursor_pos(), 1);
        editor.cursor_end();
        assert_eq!(editor.cursor_pos(), 5);
        editor.cursor_left();
        assert_eq!(editor.cursor_pos(), 4);
    }

    #[test]
    fn test_history() {
        let mut editor = LineEditor::new();
        editor.add_to_history("first".to_string());
        editor.add_to_history("second".to_string());

        assert!(editor.history_up());
        assert_eq!(editor.buffer(), "second");
        assert!(editor.history_up());
        assert_eq!(editor.buffer(), "first");
        assert!(!editor.history_up()); // No more history

        assert!(editor.history_down());
        assert_eq!(editor.buffer(), "second");
    }

    #[test]
    fn test_unicode() {
        let mut editor = LineEditor::new();
        // Test with actual unicode characters
        editor.insert_str("hello");
        assert_eq!(editor.buffer(), "hello");
        editor.cursor_home();
        editor.cursor_right(); // 'h' -> 'e'
        editor.cursor_right(); // 'e' -> 'l'
        assert_eq!(editor.cursor_pos(), 2);

        // Test with multi-byte characters
        editor.clear();
        editor.insert_char('\u{1F600}'); // Grinning face emoji (4 bytes)
        assert_eq!(editor.cursor_pos(), 4); // emoji is 4 bytes UTF-8
        assert_eq!(editor.buffer().len(), 4);
    }

    #[test]
    fn test_submit() {
        let mut editor = LineEditor::new();
        editor.insert_str("test command");
        let line = editor.submit();
        assert_eq!(line, "test command");
        assert_eq!(editor.buffer(), "");
        assert_eq!(editor.cursor_pos(), 0);
    }
}
