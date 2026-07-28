use crate::observation::translator::RawObservation;
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Filter Decision (RFC-0003, Section 7)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    Accept,
    Reject { reason: String },
}

// ---------------------------------------------------------------------------
// Observation Filter
// ---------------------------------------------------------------------------

pub struct ObservationFilter {
    root: PathBuf,
    ignore_patterns: Vec<String>,
}

impl ObservationFilter {
    pub fn new(root: PathBuf) -> Self {
        Self {
            root,
            ignore_patterns: vec![
                ".git".to_string(),
                "target".to_string(),
                ".DS_Store".to_string(),
                "Thumbs.db".to_string(),
            ],
        }
    }

    /// Add an ignore pattern.
    pub fn add_ignore_pattern(&mut self, pattern: String) {
        self.ignore_patterns.push(pattern);
    }

    /// Filter a raw observation.
    pub fn filter(&self, observation: &RawObservation) -> FilterDecision {
        let path = &observation.path;

        // Check if path is within root
        if let Ok(relative) = path.strip_prefix(&self.root) {
            // Check ignore patterns
            for component in relative.components() {
                let comp_str = component.as_os_str().to_string_lossy().to_string();

                // Check exact match
                if self.ignore_patterns.contains(&comp_str) {
                    return FilterDecision::Reject {
                        reason: format!("matches ignore pattern: {}", comp_str),
                    };
                }

                // Check prefix match (for dotfiles)
                if comp_str.starts_with('.') && comp_str.len() > 1 {
                    return FilterDecision::Reject {
                        reason: format!("hidden file: {}", comp_str),
                    };
                }

                // Check suffix match (for temp files)
                if comp_str.ends_with('~') {
                    return FilterDecision::Reject {
                        reason: format!("temp file: {}", comp_str),
                    };
                }
                if comp_str.ends_with(".swp") || comp_str.ends_with(".swo") {
                    return FilterDecision::Reject {
                        reason: format!("swap file: {}", comp_str),
                    };
                }
            }

            FilterDecision::Accept
        } else {
            FilterDecision::Reject {
                reason: "path outside project root".to_string(),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::observation::translator::RawOperation;

    fn test_filter() -> ObservationFilter {
        ObservationFilter::new(PathBuf::from("/project"))
    }

    fn accept_obs(path: &str) -> RawObservation {
        RawObservation {
            path: PathBuf::from(path),
            operation: RawOperation::Created,
        }
    }

    #[test]
    fn accept_normal_file() {
        let filter = test_filter();
        let obs = accept_obs("/project/src/main.rs");
        assert_eq!(filter.filter(&obs), FilterDecision::Accept);
    }

    #[test]
    fn reject_git_directory() {
        let filter = test_filter();
        let obs = accept_obs("/project/.git/config");
        assert!(matches!(
            filter.filter(&obs),
            FilterDecision::Reject { .. }
        ));
    }

    #[test]
    fn reject_target_directory() {
        let filter = test_filter();
        let obs = accept_obs("/project/target/debug/binary");
        assert!(matches!(
            filter.filter(&obs),
            FilterDecision::Reject { .. }
        ));
    }

    #[test]
    fn reject_hidden_files() {
        let filter = test_filter();
        let obs = accept_obs("/project/.env");
        assert!(matches!(
            filter.filter(&obs),
            FilterDecision::Reject { .. }
        ));
    }

    #[test]
    fn reject_temp_files() {
        let filter = test_filter();
        let obs = accept_obs("/project/main.rs~");
        assert!(matches!(
            filter.filter(&obs),
            FilterDecision::Reject { .. }
        ));
    }

    #[test]
    fn reject_swap_files() {
        let filter = test_filter();
        let obs = accept_obs("/project/main.rs.swp");
        assert!(matches!(
            filter.filter(&obs),
            FilterDecision::Reject { .. }
        ));
    }

    #[test]
    fn reject_path_outside_root() {
        let filter = test_filter();
        let obs = accept_obs("/other/file.txt");
        assert!(matches!(
            filter.filter(&obs),
            FilterDecision::Reject { .. }
        ));
    }

    #[test]
    fn custom_ignore_pattern() {
        let mut filter = test_filter();
        filter.add_ignore_pattern("node_modules".to_string());

        let obs = accept_obs("/project/node_modules/package/index.js");
        assert!(matches!(
            filter.filter(&obs),
            FilterDecision::Reject { .. }
        ));
    }
}
