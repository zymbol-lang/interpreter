//! Source position tracking for Zymbol-Lang
//!
//! This crate provides types and utilities for tracking source code positions
//! throughout the compilation pipeline. It supports Unicode properly, including
//! emoji characters in identifiers.

use std::collections::HashMap;

/// Unique identifier for a source file
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FileId(pub usize);

/// A position in source code (line and column)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Position {
    /// Line number (1-indexed)
    pub line: u32,
    /// Column number (1-indexed, counts Unicode grapheme clusters)
    pub column: u32,
    /// Byte offset from start of file (0-indexed)
    pub byte_offset: u32,
}

impl Position {
    pub fn new(line: u32, column: u32, byte_offset: u32) -> Self {
        Self {
            line,
            column,
            byte_offset,
        }
    }

    /// Create a position at the start of a file
    pub fn start() -> Self {
        Self {
            line: 1,
            column: 1,
            byte_offset: 0,
        }
    }
}

/// A span of source code between two positions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// Starting position (inclusive)
    pub start: Position,
    /// Ending position (exclusive)
    pub end: Position,
    /// File this span belongs to
    pub file_id: FileId,
}

impl Span {
    pub fn new(start: Position, end: Position, file_id: FileId) -> Self {
        Self {
            start,
            end,
            file_id,
        }
    }

    /// Create a span that covers from this span to another
    pub fn to(&self, other: &Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
            file_id: self.file_id,
        }
    }

    /// Get the length of this span in bytes
    pub fn len(&self) -> usize {
        (self.end.byte_offset - self.start.byte_offset) as usize
    }

    /// Check if this span is empty
    pub fn is_empty(&self) -> bool {
        self.start.byte_offset == self.end.byte_offset
    }
}

/// A source file with its content and metadata
#[derive(Debug, Clone)]
pub struct SourceFile {
    /// Unique identifier for this file
    pub id: FileId,
    /// File name or path
    pub name: String,
    /// Source code content
    pub source: String,
    /// Byte offsets of line starts (for quick line lookup)
    line_starts: Vec<u32>,
}

impl SourceFile {
    /// Create a new source file and compute line starts
    pub fn new(id: FileId, name: String, source: String) -> Self {
        let line_starts = compute_line_starts(&source);
        Self {
            id,
            name,
            source,
            line_starts,
        }
    }

    /// Get the line number for a byte offset
    pub fn line_number(&self, byte_offset: u32) -> u32 {
        match self.line_starts.binary_search(&byte_offset) {
            Ok(line) => line as u32 + 1,
            Err(next_line) => next_line as u32,
        }
    }

    /// Get the column number for a byte offset
    pub fn column_number(&self, byte_offset: u32) -> u32 {
        let line_num = self.line_number(byte_offset);
        let line_start = self.line_starts[(line_num - 1) as usize];

        // Count grapheme clusters from line start to byte offset
        use unicode_segmentation::UnicodeSegmentation;
        let line_content = &self.source[line_start as usize..byte_offset as usize];
        line_content.graphemes(true).count() as u32 + 1
    }

    /// Convert a byte offset to a Position
    pub fn position(&self, byte_offset: u32) -> Position {
        Position {
            line: self.line_number(byte_offset),
            column: self.column_number(byte_offset),
            byte_offset,
        }
    }

    /// Get the source text for a given span
    pub fn snippet(&self, span: &Span) -> &str {
        let start = span.start.byte_offset as usize;
        let end = span.end.byte_offset as usize;
        &self.source[start..end]
    }

    /// Get a full line of source code
    pub fn line(&self, line_number: u32) -> Option<&str> {
        if line_number == 0 || line_number as usize > self.line_starts.len() {
            return None;
        }

        let start = self.line_starts[(line_number - 1) as usize] as usize;
        let end = if line_number as usize == self.line_starts.len() {
            self.source.len()
        } else {
            self.line_starts[line_number as usize] as usize
        };

        Some(self.source[start..end].trim_end_matches(['\r', '\n']))
    }
}

/// Compute byte offsets of line starts in source code
fn compute_line_starts(source: &str) -> Vec<u32> {
    let mut line_starts = vec![0];

    for (idx, ch) in source.char_indices() {
        if ch == '\n' {
            line_starts.push((idx + ch.len_utf8()) as u32);
        }
    }

    line_starts
}

/// Maps FileId to SourceFile, managing all source files in compilation
#[derive(Debug, Default)]
pub struct SourceMap {
    files: HashMap<FileId, SourceFile>,
    next_id: usize,
}

impl SourceMap {
    pub fn new() -> Self {
        Self {
            files: HashMap::new(),
            next_id: 0,
        }
    }

    /// Add a new source file and return its FileId
    pub fn add_file(&mut self, name: String, source: String) -> FileId {
        let id = FileId(self.next_id);
        self.next_id += 1;

        let file = SourceFile::new(id, name, source);
        self.files.insert(id, file);

        id
    }

    /// Get a source file by its ID
    pub fn get(&self, id: FileId) -> Option<&SourceFile> {
        self.files.get(&id)
    }

    /// Get the source snippet for a span
    pub fn snippet(&self, span: &Span) -> Option<&str> {
        self.files.get(&span.file_id).map(|f| f.snippet(span))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_position() {
        let pos = Position::new(10, 5, 123);
        assert_eq!(pos.line, 10);
        assert_eq!(pos.column, 5);
        assert_eq!(pos.byte_offset, 123);
    }

    #[test]
    fn test_span() {
        let start = Position::new(1, 1, 0);
        let end = Position::new(1, 5, 4);
        let span = Span::new(start, end, FileId(0));

        assert_eq!(span.len(), 4);
        assert!(!span.is_empty());
    }

    #[test]
    fn test_source_file() {
        let source = "hello\nworld\n🚀";
        let file = SourceFile::new(FileId(0), "test.z".to_string(), source.to_string());

        // Line numbers
        assert_eq!(file.line_number(0), 1);
        assert_eq!(file.line_number(5), 1);
        assert_eq!(file.line_number(6), 2);

        // Get lines
        assert_eq!(file.line(1), Some("hello"));
        assert_eq!(file.line(2), Some("world"));
        assert_eq!(file.line(3), Some("🚀"));
    }

    #[test]
    fn test_source_file_unicode() {
        // Test with emoji identifier
        let source = "nombre = 😀";
        let file = SourceFile::new(FileId(0), "emoji.z".to_string(), source.to_string());

        // The emoji is at byte offset 9
        let emoji_offset = 9;
        assert_eq!(file.line_number(emoji_offset as u32), 1);

        // Column should account for grapheme clusters
        let col = file.column_number(emoji_offset as u32);
        assert!(col > 0);
    }

    #[test]
    fn test_source_map() {
        let mut map = SourceMap::new();

        let id1 = map.add_file("file1.z".to_string(), "x = 5".to_string());
        let id2 = map.add_file("file2.z".to_string(), "y = 10".to_string());

        assert_ne!(id1, id2);
        assert!(map.get(id1).is_some());
        assert!(map.get(id2).is_some());

        let file1 = map.get(id1).unwrap();
        assert_eq!(file1.name, "file1.z");
    }

    #[test]
    fn test_line_starts() {
        let source = "line1\nline2\nline3";
        let starts = compute_line_starts(source);

        assert_eq!(starts.len(), 3);
        assert_eq!(starts[0], 0);     // "line1\n"
        assert_eq!(starts[1], 6);     // "line2\n"
        assert_eq!(starts[2], 12);    // "line3"
    }
}
