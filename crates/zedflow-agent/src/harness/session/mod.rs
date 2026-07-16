//! Session tree, storage, and repository implementations.

#[path = "jsonl-repo.rs"]
pub mod jsonl_repo;
#[path = "jsonl-storage.rs"]
pub mod jsonl_storage;
#[path = "memory-repo.rs"]
pub mod memory_repo;
#[path = "memory-storage.rs"]
pub mod memory_storage;
#[path = "repo-utils.rs"]
pub mod repo_utils;
pub mod session;
pub mod uuid;

pub use jsonl_repo::JsonlSessionRepo;
pub use jsonl_storage::{
    JsonlSessionStorage, JsonlSessionStorageCreateOptions, JsonlSessionStorageFileSystem,
    load_jsonl_session_metadata,
};
pub use memory_repo::InMemorySessionRepo;
pub use memory_storage::{InMemorySessionStorage, InMemorySessionStorageOptions};
pub use repo_utils::{
    create_session_id, create_timestamp, get_entries_to_fork, get_file_system_result_or_throw,
    to_session,
};
pub use session::{Session, build_session_context};
pub use uuid::uuidv7;
