use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::RuntimeError;

// ---------------------------------------------------------------------------
// WorkspaceStatus
//
// Enum describing workspace state. Deterministic serialization,
// safe hashing, compiler-enforced exhaustiveness.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum WorkspaceStatus {
    Clean,
    Modified,
    Failed,
    Unknown,
}

// ---------------------------------------------------------------------------
// InferenceContext
//
// Structured data extracted from kernel projections. Value type
// with deterministic hash for replay and caching.
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InferenceContext {
    pub recent_changes: Vec<String>,
    pub workspace_status: WorkspaceStatus,
    pub file_count: usize,
    pub hash: u64,
}

impl Default for InferenceContext {
    fn default() -> Self {
        Self {
            recent_changes: Vec::new(),
            workspace_status: WorkspaceStatus::Unknown,
            file_count: 0,
            hash: 0,
        }
    }
}

impl InferenceContext {
    pub fn compute_hash(&self) -> u64 {
        compute_hash(&self.recent_changes, self.workspace_status, self.file_count)
    }
}

// ---------------------------------------------------------------------------
// ContextSource
//
// Read-only trait that produces InferenceContext. Implementations
// read from whatever data source they wrap (kernel, null, mock).
// The runtime never mutates workspace state through this trait.
// ---------------------------------------------------------------------------

pub trait ContextSource: Send + Sync {
    fn context(&self) -> Result<InferenceContext, RuntimeError>;
}

// ---------------------------------------------------------------------------
// NullContextSource
//
// Fallback when no live workspace is available. Returns the
// canonical "no workspace" InferenceContext::default().
// ---------------------------------------------------------------------------

pub struct NullContextSource;

impl ContextSource for NullContextSource {
    fn context(&self) -> Result<InferenceContext, RuntimeError> {
        Ok(InferenceContext::default())
    }
}

// ---------------------------------------------------------------------------
// Hash computation
//
// Deterministic hash that sorts recent changes to ensure
// ordering independence.
// ---------------------------------------------------------------------------

fn compute_hash(changes: &[String], status: WorkspaceStatus, file_count: usize) -> u64 {
    let mut sorted: Vec<&String> = changes.iter().collect();
    sorted.sort();

    let mut hasher = DefaultHasher::new();

    // Hash status as discriminant
    (status as u8).hash(&mut hasher);

    // Hash file count
    file_count.hash(&mut hasher);

    // Hash sorted changes
    for change in sorted {
        change.hash(&mut hasher);
    }

    hasher.finish()
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_status_is_clone() {
        let status = WorkspaceStatus::Clean;
        let cloned = status;
        assert_eq!(status, cloned);
    }

    #[test]
    fn workspace_status_is_debug() {
        let status = WorkspaceStatus::Modified;
        let _ = format!("{:?}", status);
    }

    #[test]
    fn inference_context_default() {
        let ctx = InferenceContext::default();
        assert!(ctx.recent_changes.is_empty());
        assert_eq!(ctx.workspace_status, WorkspaceStatus::Unknown);
        assert_eq!(ctx.file_count, 0);
        assert_eq!(ctx.hash, 0);
    }

    #[test]
    fn inference_context_starts_with_hash() {
        let ctx = InferenceContext {
            recent_changes: vec!["a.rs".into()],
            workspace_status: WorkspaceStatus::Clean,
            file_count: 1,
            hash: 0,
        };
        let computed = ctx.compute_hash();
        assert_ne!(computed, 0);
    }

    #[test]
    fn hash_deterministic() {
        let changes = vec!["main.rs".into(), "lib.rs".into()];
        let h1 = compute_hash(&changes, WorkspaceStatus::Modified, 2);
        let h2 = compute_hash(&changes, WorkspaceStatus::Modified, 2);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_sort_independent() {
        let a = vec!["main.rs".into(), "lib.rs".into()];
        let b = vec!["lib.rs".into(), "main.rs".into()];
        let h1 = compute_hash(&a, WorkspaceStatus::Modified, 2);
        let h2 = compute_hash(&b, WorkspaceStatus::Modified, 2);
        assert_eq!(h1, h2);
    }

    #[test]
    fn hash_differs_by_status() {
        let changes = vec!["a.rs".into()];
        let h1 = compute_hash(&changes, WorkspaceStatus::Clean, 1);
        let h2 = compute_hash(&changes, WorkspaceStatus::Modified, 1);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_differs_by_file_count() {
        let changes = vec!["a.rs".into()];
        let h1 = compute_hash(&changes, WorkspaceStatus::Modified, 1);
        let h2 = compute_hash(&changes, WorkspaceStatus::Modified, 2);
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_differs_by_changes() {
        let a = vec!["a.rs".into()];
        let b = vec!["b.rs".into()];
        let h1 = compute_hash(&a, WorkspaceStatus::Modified, 1);
        let h2 = compute_hash(&b, WorkspaceStatus::Modified, 1);
        assert_ne!(h1, h2);
    }

    #[test]
    fn null_context_source_returns_default() {
        let source = NullContextSource;
        let ctx = source.context().unwrap();
        assert_eq!(ctx, InferenceContext::default());
    }
}
