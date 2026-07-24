//! NFO read + import-on-refresh (Swift MediaCore/NFO + LibraryRefreshService).

mod import;
mod reader;

pub use import::import_nfo_for_item;
pub use reader::{NfoError, NfoParsedData, NfoReader};
