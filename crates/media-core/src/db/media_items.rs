use chrono::{DateTime, Utc};
use rusqlite::params;

use crate::models::{MediaItem, MediaType, ScrapedStatus};
use crate::DatabaseError;

use super::AppDatabase;

impl AppDatabase {
    pub fn list_media_items(&self, library_id: &str) -> Result<Vec<MediaItem>, DatabaseError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, type, title, originalTitle, year, folderPath, filePath,
                        bookmarkData, status, scrapeIssue, libraryId, addedAt
                 FROM media_items
                 WHERE libraryId = ?1
                 ORDER BY title COLLATE NOCASE ASC",
            )?;
            let rows = stmt.query_map([library_id], map_media_item)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn list_media_file_paths(&self, library_id: &str) -> Result<Vec<String>, DatabaseError> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT filePath FROM media_items WHERE libraryId = ?1")?;
            let rows = stmt.query_map([library_id], |row| row.get(0))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn list_media_folder_paths(&self, library_id: &str) -> Result<Vec<String>, DatabaseError> {
        self.with_conn(|conn| {
            let mut stmt =
                conn.prepare("SELECT folderPath FROM media_items WHERE libraryId = ?1")?;
            let rows = stmt.query_map([library_id], |row| row.get(0))?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    /// Persist new media items in a single transaction (F1).
    pub fn insert_media_items(&self, items: &[MediaItem]) -> Result<(), DatabaseError> {
        if items.is_empty() {
            return Ok(());
        }
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut stmt = tx.prepare(
                    "INSERT INTO media_items (
                        id, type, title, originalTitle, year, folderPath, filePath,
                        bookmarkData, status, scrapeIssue, libraryId, addedAt
                     ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)",
                )?;
                for item in items {
                    stmt.execute(params![
                        item.id,
                        item.media_type.as_str(),
                        item.title,
                        item.original_title,
                        item.year,
                        item.folder_path,
                        item.file_path,
                        item.bookmark_data,
                        item.status.as_str(),
                        item.scrape_issue,
                        item.library_id,
                        item.added_at.to_rfc3339(),
                    ])?;
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn delete_media_items_under_folders(
        &self,
        library_id: &str,
        folder_paths: &[String],
    ) -> Result<usize, DatabaseError> {
        if folder_paths.is_empty() {
            return Ok(0);
        }
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            let mut deleted = 0usize;
            {
                let mut stmt = tx.prepare(
                    "DELETE FROM media_items WHERE libraryId = ?1 AND folderPath = ?2",
                )?;
                for folder in folder_paths {
                    deleted += stmt.execute(params![library_id, folder])? as usize;
                }
            }
            tx.commit()?;
            Ok(deleted)
        })
    }

    pub fn update_status(
        &self,
        item_id: &str,
        status: ScrapedStatus,
        scrape_issue: Option<&str>,
    ) -> Result<(), DatabaseError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE media_items SET status = ?2, scrapeIssue = ?3 WHERE id = ?1",
                params![item_id, status.as_str(), scrape_issue],
            )?;
            Ok(())
        })
    }

    pub fn update_title(
        &self,
        item_id: &str,
        title: &str,
        original_title: Option<&str>,
    ) -> Result<(), DatabaseError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE media_items SET title = ?2, originalTitle = ?3 WHERE id = ?1",
                params![item_id, title, original_title],
            )?;
            Ok(())
        })
    }

    pub fn update_year(&self, item_id: &str, year: Option<i32>) -> Result<(), DatabaseError> {
        self.with_conn(|conn| {
            conn.execute(
                "UPDATE media_items SET year = ?2 WHERE id = ?1",
                params![item_id, year],
            )?;
            Ok(())
        })
    }
}

fn map_media_item(row: &rusqlite::Row<'_>) -> rusqlite::Result<MediaItem> {
    let media_type: String = row.get(1)?;
    let status: String = row.get(8)?;
    let added_at: String = row.get(11)?;
    Ok(MediaItem {
        id: row.get(0)?,
        media_type: MediaType::parse(&media_type).unwrap_or(MediaType::Movie),
        title: row.get(2)?,
        original_title: row.get(3)?,
        year: row.get(4)?,
        folder_path: row.get(5)?,
        file_path: row.get(6)?,
        bookmark_data: row.get(7)?,
        status: ScrapedStatus::parse(&status).unwrap_or(ScrapedStatus::Unscraped),
        scrape_issue: row.get(9)?,
        library_id: row.get(10)?,
        added_at: DateTime::parse_from_rfc3339(&added_at)
            .map(|dt| dt.with_timezone(&Utc))
            .unwrap_or_else(|_| Utc::now()),
    })
}
