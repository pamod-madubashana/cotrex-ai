use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

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

impl InferenceContext {
    pub fn compute_hash(&self) -> u64 {
        compute_hash(&self.recent_changes, self.workspace_status, self.file_count)
    }
}

// ---------------------------------------------------------------------------
// ContextBuilder
//
// Trait that reads projection state and produces InferenceContext.
// ---------------------------------------------------------------------------

pub trait ContextBuilder {
    fn build_context(&self, recent_changes: &[String], file_count: usize) -> InferenceContext;
}

// ---------------------------------------------------------------------------
// DefaultContextBuilder
//
// Default implementation that extracts fields and computes hash.
// Sorts recent changes to ensure ordering independence.
// ---------------------------------------------------------------------------

pub struct DefaultContextBuilder;

impl ContextBuilder for DefaultContextBuilder {
    fn build_context(&self, recent_changes: &[String], file_count: usize) -> InferenceContext {
        let mut changes: Vec<String> = recent_changes.to_vec();
        changes.sort();

        let status = if file_count == 0 {
            WorkspaceStatus::Unknown
        } else {
            WorkspaceStatus::Modified
        };

        let hash = compute_hash(&changes, status, file_count);

        InferenceContext {
            recent_changes: changes,
            workspace_status: status,
            file_count,
            hash,
        }
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
    fn default_context_builder_sorts_changes() {
        let builder = DefaultContextBuilder;
        let changes: Vec<String> = vec!["z.rs".into(), "a.rs".into(), "m.rs".into()];
        let ctx = builder.build_context(&changes, 3);
        let expected: Vec<String> = vec!["a.rs".into(), "m.rs".into(), "z.rs".into()];
        assert_eq!(ctx.recent_changes, expected);
    }

    #[test]
    fn default_context_builder_empty_is_unknown() {
        let builder = DefaultContextBuilder;
        let ctx = builder.build_context(&[], 0);
        assert_eq!(ctx.workspace_status, WorkspaceStatus::Unknown);
    }

    #[test]
    fn default_context_builder_nonempty_is_modified() {
        let builder = DefaultContextBuilder;
        let ctx = builder.build_context(&["a.rs".into()], 1);
        assert_eq!(ctx.workspace_status, WorkspaceStatus::Modified);
    }
}
