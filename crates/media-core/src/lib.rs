//! ScrapeX media domain core.
//!
//! M0: models, SQLite schema/migrations, filesystem mutation layer.

pub mod db;
pub mod filesystem;
pub mod models;

pub use db::{AppDatabase, DatabaseError};
pub use filesystem::{
    CollisionPolicy, FilesystemChangeSet, FilesystemError, FilesystemMoveRecord,
    FilesystemService, RemovalStrategy, WriteOptions,
};
pub use models::{Library, MediaItem, MediaType, ScrapedStatus};
