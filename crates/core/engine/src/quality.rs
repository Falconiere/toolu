//! Shared post-edit quality flow for language rule crates (#459).

mod edit;

pub use edit::{EditedFile, edited_file, in_linked_worktree, is_regular_file};
