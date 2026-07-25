//! NFO read + import-on-refresh + Kodi/Emby write (M2/M4).

mod import;
mod reader;
mod writer;

pub use import::import_nfo_for_item;
pub use reader::{NfoError, NfoParsedData, NfoReader};
pub use writer::{write_emby_nfo, write_kodi_nfo, write_nfo};
