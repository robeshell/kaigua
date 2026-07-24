//! NFO read + import-on-refresh + Kodi write (M2).

mod import;
mod reader;
mod writer;

pub use import::import_nfo_for_item;
pub use reader::{NfoError, NfoParsedData, NfoReader};
pub use writer::write_kodi_nfo;
