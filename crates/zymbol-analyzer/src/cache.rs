//! Thread-safe document cache for Zymbol-Lang analyzer
//!
//! Uses DashMap for lock-free concurrent reads, optimized for
//! the read-heavy workload typical of LSP servers.

use dashmap::DashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use zymbol_span::FileId;

use crate::document::Document;

/// Thread-safe document cache
///
/// Provides concurrent access to documents with efficient read operations.
/// Documents are stored by their URI, and the cache handles FileId allocation.
#[derive(Debug)]
pub struct DocumentCache {
    /// Map from URI to Document
    documents: DashMap<Arc<str>, Document>,
    /// Counter for generating unique FileIds
    next_file_id: AtomicUsize,
}

impl DocumentCache {
    /// Create a new empty document cache
    pub fn new() -> Self {
        Self {
            documents: DashMap::new(),
            next_file_id: AtomicUsize::new(0),
        }
    }

    /// Allocate a new unique FileId
    fn allocate_file_id(&self) -> FileId {
        FileId(self.next_file_id.fetch_add(1, Ordering::Relaxed))
    }

    /// Open a new document or update an existing one
    ///
    /// Returns the FileId assigned to this document
    pub fn open(&self, uri: Arc<str>, content: String, version: i32) -> FileId {
        // Check if document already exists
        if let Some(existing) = self.documents.get(&uri) {
            // If same version, no need to update
            if existing.version == version {
                return existing.file_id();
            }
        }

        // Create new document with a fresh FileId
        let file_id = self.allocate_file_id();
        let document = Document::new(uri.clone(), content, version, file_id);
        self.documents.insert(uri, document);
        file_id
    }

    /// Update an existing document's content
    ///
    /// Returns the new FileId if the document exists, None otherwise
    pub fn update(&self, uri: &str, content: String, version: i32) -> Option<FileId> {
        let uri_arc: Arc<str> = Arc::from(uri);

        if self.documents.contains_key(&uri_arc) {
            let file_id = self.allocate_file_id();
            let document = Document::new(uri_arc.clone(), content, version, file_id);
            self.documents.insert(uri_arc, document);
            Some(file_id)
        } else {
            None
        }
    }

    /// Close a document (remove from cache)
    pub fn close(&self, uri: &str) {
        let uri_arc: Arc<str> = Arc::from(uri);
        self.documents.remove(&uri_arc);
    }

    /// Get a document by URI
    ///
    /// Returns a reference guard that allows reading the document
    pub fn get(&self, uri: &str) -> Option<dashmap::mapref::one::Ref<'_, Arc<str>, Document>> {
        let uri_arc: Arc<str> = Arc::from(uri);
        self.documents.get(&uri_arc)
    }

    /// Check if a document exists
    pub fn contains(&self, uri: &str) -> bool {
        let uri_arc: Arc<str> = Arc::from(uri);
        self.documents.contains_key(&uri_arc)
    }

    /// Get the number of documents in the cache
    pub fn len(&self) -> usize {
        self.documents.len()
    }

    /// Check if the cache is empty
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }

    /// Iterate over all document URIs
    pub fn uris(&self) -> Vec<Arc<str>> {
        self.documents.iter().map(|r| r.key().clone()).collect()
    }

    /// Clear all documents from the cache
    pub fn clear(&self) {
        self.documents.clear();
    }
}

impl Default for DocumentCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_creation() {
        let cache = DocumentCache::new();
        assert!(cache.is_empty());
        assert_eq!(cache.len(), 0);
    }

    #[test]
    fn test_open_document() {
        let cache = DocumentCache::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        let file_id = cache.open(uri.clone(), "x = 5".to_string(), 1);
        assert_eq!(file_id.0, 0);
        assert_eq!(cache.len(), 1);
        assert!(cache.contains("file:///test.zy"));
    }

    #[test]
    fn test_update_document() {
        let cache = DocumentCache::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        cache.open(uri.clone(), "x = 5".to_string(), 1);

        // Update with new content
        let new_file_id = cache.update("file:///test.zy", "x = 10".to_string(), 2);
        assert!(new_file_id.is_some());
        assert_eq!(new_file_id.unwrap().0, 1); // New FileId allocated

        // Verify content changed
        let doc = cache.get("file:///test.zy").unwrap();
        assert_eq!(doc.version, 2);
        assert_eq!(&*doc.content, "x = 10");
    }

    #[test]
    fn test_close_document() {
        let cache = DocumentCache::new();
        let uri: Arc<str> = Arc::from("file:///test.zy");

        cache.open(uri.clone(), "x = 5".to_string(), 1);
        assert_eq!(cache.len(), 1);

        cache.close("file:///test.zy");
        assert_eq!(cache.len(), 0);
        assert!(!cache.contains("file:///test.zy"));
    }

    #[test]
    fn test_multiple_documents() {
        let cache = DocumentCache::new();

        cache.open(Arc::from("file:///a.zy"), "a = 1".to_string(), 1);
        cache.open(Arc::from("file:///b.zy"), "b = 2".to_string(), 1);
        cache.open(Arc::from("file:///c.zy"), "c = 3".to_string(), 1);

        assert_eq!(cache.len(), 3);

        let uris = cache.uris();
        assert_eq!(uris.len(), 3);
    }

    #[test]
    fn test_file_id_uniqueness() {
        let cache = DocumentCache::new();

        let id1 = cache.open(Arc::from("file:///a.zy"), "a = 1".to_string(), 1);
        let id2 = cache.open(Arc::from("file:///b.zy"), "b = 2".to_string(), 1);
        let id3 = cache.open(Arc::from("file:///c.zy"), "c = 3".to_string(), 1);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);
    }

    #[test]
    fn test_update_nonexistent() {
        let cache = DocumentCache::new();
        let result = cache.update("file:///nonexistent.zy", "x = 5".to_string(), 1);
        assert!(result.is_none());
    }
}
