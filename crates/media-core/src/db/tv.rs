use rusqlite::{params, OptionalExtension};

use crate::models::{CastMember, TvEpisode, TvSeason};
use crate::scanner::ScannedEpisode;
use crate::DatabaseError;

use super::AppDatabase;

impl AppDatabase {
    /// Insert seasons/episodes for newly scanned shows in one transaction.
    pub fn insert_show_episodes(
        &self,
        media_item_id: &str,
        episodes: &[ScannedEpisode],
    ) -> Result<(), DatabaseError> {
        if episodes.is_empty() {
            return Ok(());
        }
        self.with_conn(|conn| {
            let tx = conn.unchecked_transaction()?;
            {
                let mut by_season: std::collections::HashMap<i32, Vec<&ScannedEpisode>> =
                    std::collections::HashMap::new();
                for ep in episodes {
                    by_season.entry(ep.season).or_default().push(ep);
                }

                let mut season_stmt = tx.prepare(
                    "INSERT OR REPLACE INTO tv_seasons (
                        id, mediaItemId, seasonNumber, title, overview, posterPath, airDate, episodeCount
                     ) VALUES (?1, ?2, ?3, NULL, NULL, NULL, NULL, ?4)",
                )?;
                let mut episode_stmt = tx.prepare(
                    "INSERT OR REPLACE INTO tv_episodes (
                        id, seasonId, episodeNumber, title, overview, airDate, stillPath, stillURL,
                        filePath, runtime, rating, director, writer, guestCast, absoluteNumber,
                        finaleType, videoCodec, videoResolution, audioCodec
                     ) VALUES (
                        ?1, ?2, ?3, ?4, NULL, NULL, NULL, NULL,
                        ?5, NULL, NULL, NULL, NULL, '[]', NULL,
                        NULL, NULL, NULL, NULL
                     )",
                )?;

                for (season_num, eps) in by_season {
                    let season_id = format!("{media_item_id}_S{season_num}");
                    season_stmt.execute(params![
                        season_id,
                        media_item_id,
                        season_num,
                        eps.len() as i64
                    ])?;
                    for ep in eps {
                        let episode_id = format!("{season_id}_E{}", ep.episode);
                        let title = if ep.title.is_empty() {
                            None
                        } else {
                            Some(ep.title.as_str())
                        };
                        episode_stmt.execute(params![
                            episode_id,
                            season_id,
                            ep.episode,
                            title,
                            ep.file_path,
                        ])?;
                    }
                }
            }
            tx.commit()?;
            Ok(())
        })
    }

    pub fn fetch_seasons(&self, media_item_id: &str) -> Result<Vec<TvSeason>, DatabaseError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, mediaItemId, seasonNumber, title, overview, posterPath, airDate, episodeCount
                 FROM tv_seasons
                 WHERE mediaItemId = ?1
                 ORDER BY seasonNumber ASC",
            )?;
            let rows = stmt.query_map([media_item_id], map_season)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn fetch_episodes(&self, season_id: &str) -> Result<Vec<TvEpisode>, DatabaseError> {
        self.with_conn(|conn| {
            let mut stmt = conn.prepare(
                "SELECT id, seasonId, episodeNumber, title, overview, airDate, stillPath, stillURL,
                        filePath, runtime, rating, director, writer, guestCast, absoluteNumber,
                        finaleType, videoCodec, videoResolution, audioCodec
                 FROM tv_episodes
                 WHERE seasonId = ?1
                 ORDER BY episodeNumber ASC",
            )?;
            let rows = stmt.query_map([season_id], map_episode)?;
            let mut out = Vec::new();
            for row in rows {
                out.push(row?);
            }
            Ok(out)
        })
    }

    pub fn upsert_season(&self, season: &TvSeason) -> Result<(), DatabaseError> {
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO tv_seasons (
                    id, mediaItemId, seasonNumber, title, overview, posterPath, airDate, episodeCount
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                 ON CONFLICT(id) DO UPDATE SET
                    mediaItemId=excluded.mediaItemId,
                    seasonNumber=excluded.seasonNumber,
                    title=excluded.title,
                    overview=excluded.overview,
                    posterPath=excluded.posterPath,
                    airDate=excluded.airDate,
                    episodeCount=excluded.episodeCount",
                params![
                    season.id,
                    season.media_item_id,
                    season.season_number,
                    season.title,
                    season.overview,
                    season.poster_path,
                    season.air_date,
                    season.episode_count,
                ],
            )?;
            Ok(())
        })
    }

    pub fn upsert_episode(&self, episode: &TvEpisode) -> Result<(), DatabaseError> {
        let guest_cast =
            serde_json::to_string(&episode.guest_cast).unwrap_or_else(|_| "[]".into());
        self.with_conn(|conn| {
            conn.execute(
                "INSERT INTO tv_episodes (
                    id, seasonId, episodeNumber, title, overview, airDate, stillPath, stillURL,
                    filePath, runtime, rating, director, writer, guestCast, absoluteNumber,
                    finaleType, videoCodec, videoResolution, audioCodec
                 ) VALUES (
                    ?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                    ?9, ?10, ?11, ?12, ?13, ?14, ?15,
                    ?16, ?17, ?18, ?19
                 )
                 ON CONFLICT(id) DO UPDATE SET
                    seasonId=excluded.seasonId,
                    episodeNumber=excluded.episodeNumber,
                    title=excluded.title,
                    overview=excluded.overview,
                    airDate=excluded.airDate,
                    stillPath=excluded.stillPath,
                    stillURL=excluded.stillURL,
                    filePath=excluded.filePath,
                    runtime=excluded.runtime,
                    rating=excluded.rating,
                    director=excluded.director,
                    writer=excluded.writer,
                    guestCast=excluded.guestCast,
                    absoluteNumber=excluded.absoluteNumber,
                    finaleType=excluded.finaleType,
                    videoCodec=excluded.videoCodec,
                    videoResolution=excluded.videoResolution,
                    audioCodec=excluded.audioCodec",
                params![
                    episode.id,
                    episode.season_id,
                    episode.episode_number,
                    episode.title,
                    episode.overview,
                    episode.air_date,
                    episode.still_path,
                    episode.still_url,
                    episode.file_path,
                    episode.runtime,
                    episode.rating,
                    episode.director,
                    episode.writer,
                    guest_cast,
                    episode.absolute_number,
                    episode.finale_type,
                    episode.video_codec,
                    episode.video_resolution,
                    episode.audio_codec,
                ],
            )?;
            Ok(())
        })
    }

    #[allow(dead_code)]
    pub fn get_season(&self, id: &str) -> Result<Option<TvSeason>, DatabaseError> {
        self.with_conn(|conn| {
            conn.query_row(
                "SELECT id, mediaItemId, seasonNumber, title, overview, posterPath, airDate, episodeCount
                 FROM tv_seasons WHERE id = ?1",
                [id],
                map_season,
            )
            .optional()
            .map_err(DatabaseError::from)
        })
    }
}

fn map_season(row: &rusqlite::Row<'_>) -> rusqlite::Result<TvSeason> {
    Ok(TvSeason {
        id: row.get(0)?,
        media_item_id: row.get(1)?,
        season_number: row.get(2)?,
        title: row.get(3)?,
        overview: row.get(4)?,
        poster_path: row.get(5)?,
        air_date: row.get(6)?,
        episode_count: row.get(7)?,
    })
}

fn map_episode(row: &rusqlite::Row<'_>) -> rusqlite::Result<TvEpisode> {
    let guest_cast_json: String = row.get(13)?;
    let guest_cast: Vec<CastMember> = serde_json::from_str(&guest_cast_json).unwrap_or_default();
    Ok(TvEpisode {
        id: row.get(0)?,
        season_id: row.get(1)?,
        episode_number: row.get(2)?,
        title: row.get(3)?,
        overview: row.get(4)?,
        air_date: row.get(5)?,
        still_path: row.get(6)?,
        still_url: row.get(7)?,
        file_path: row.get(8)?,
        runtime: row.get(9)?,
        rating: row.get(10)?,
        director: row.get(11)?,
        writer: row.get(12)?,
        guest_cast,
        absolute_number: row.get(14)?,
        finale_type: row.get(15)?,
        video_codec: row.get(16)?,
        video_resolution: row.get(17)?,
        audio_codec: row.get(18)?,
    })
}
