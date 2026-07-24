//! kaigua media domain core.
//!
//! M0: models, SQLite schema/migrations, filesystem mutation layer.
//! M1: library CRUD, scanning, NFO import-on-refresh.

pub mod db;
pub mod filesystem;
pub mod models;
pub mod nfo;
pub mod scanner;
pub mod thumbnail;

pub use db::{AppDatabase, DatabaseError, MediaMetaSummary};
pub use filesystem::{
    CollisionPolicy, FilesystemChangeSet, FilesystemError, FilesystemMoveRecord,
    FilesystemService, RemovalStrategy, WriteOptions,
};
pub use models::{
    CastMember, Library, MediaItem, MediaMetadata, MediaType, ScrapedStatus, TvEpisode, TvSeason,
};
pub use nfo::{import_nfo_for_item, NfoParsedData, NfoReader};
pub use scanner::{
    refresh_library, FileNameParser, RefreshReport, ScanProgress, ScannedEpisode,
};
pub use thumbnail::{ThumbnailCache, ThumbnailError};
