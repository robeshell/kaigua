use regex::Regex;
use std::sync::OnceLock;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ParsedFileName {
    pub title: String,
    pub year: Option<i32>,
    pub season: Option<i32>,
    pub episode: Option<i32>,
    pub sub_group: Option<String>,
}

pub struct FileNameParser;

impl FileNameParser {
    const MEDIA_EXTENSIONS: &[&str] = &[
        "mkv", "mp4", "avi", "m4v", "mov", "wmv", "flv", "ts", "m2ts", "iso",
    ];

    const NOISE_TOKENS: &[&str] = &[
        "1080p", "720p", "4K", "2160p", "480p", "BluRay", "BDRip", "WEB-DL", "WEBDL", "WEBRip",
        "HDRip", "HDTV", "x264", "x265", "HEVC", "AVC", "EAC3", "TrueHD", "DTS-HD", "DTS-MA",
        "DTS", "AAC", "AC3", "FLAC", "10bit", "SDR", "HDR", "HDR10", "DV", "DoVi", "Atmos",
        "REMUX", "PROPER", "HD-MA", "全集",
    ];

    pub fn parse(filename: &str) -> ParsedFileName {
        let name = Self::drop_extension(filename);
        if name.starts_with('[') {
            Self::parse_anime(&name)
        } else {
            Self::parse_standard(&name)
        }
    }

    pub fn extract_season_suffix(dir_name: &str) -> Option<(String, i32)> {
        static RE_SEASON_WORD: OnceLock<Regex> = OnceLock::new();
        static RE_S_NUM: OnceLock<Regex> = OnceLock::new();
        static RE_CN_ARABIC: OnceLock<Regex> = OnceLock::new();
        static RE_CN_HAN: OnceLock<Regex> = OnceLock::new();

        let re_season_word =
            RE_SEASON_WORD.get_or_init(|| Regex::new(r"(?i)^(.+)\s+Season\s*(\d{1,2})\s*$").unwrap());
        if let Some(caps) = re_season_word.captures(dir_name) {
            let base = caps.get(1)?.as_str().trim();
            let season: i32 = caps.get(2)?.as_str().parse().ok()?;
            if !base.is_empty() {
                return Some((base.to_string(), season));
            }
        }

        let re_s_num = RE_S_NUM.get_or_init(|| Regex::new(r"(?i)^(.+)\s+S(\d{1,2})\s*$").unwrap());
        if let Some(caps) = re_s_num.captures(dir_name) {
            let base = caps.get(1)?.as_str().trim();
            let season: i32 = caps.get(2)?.as_str().parse().ok()?;
            if !base.is_empty() {
                return Some((base.to_string(), season));
            }
        }

        let re_cn_arabic =
            RE_CN_ARABIC.get_or_init(|| Regex::new(r"^(.+?)\s*第\s*(\d{1,2})\s*季\s*$").unwrap());
        if let Some(caps) = re_cn_arabic.captures(dir_name) {
            let base = caps.get(1)?.as_str().trim();
            let season: i32 = caps.get(2)?.as_str().parse().ok()?;
            if !base.is_empty() {
                return Some((base.to_string(), season));
            }
        }

        let re_cn_han = RE_CN_HAN
            .get_or_init(|| Regex::new(r"^(.+?)\s*第\s*([一二三四五六七八九十]+)\s*季\s*$").unwrap());
        if let Some(caps) = re_cn_han.captures(dir_name) {
            let base = caps.get(1)?.as_str().trim();
            let season = chinese_numeral(caps.get(2)?.as_str())?;
            if !base.is_empty() {
                return Some((base.to_string(), season));
            }
        }

        None
    }

    fn parse_anime(name: &str) -> ParsedFileName {
        static RE_GROUP: OnceLock<Regex> = OnceLock::new();
        static RE_DASH_EP: OnceLock<Regex> = OnceLock::new();
        static RE_CN_EP: OnceLock<Regex> = OnceLock::new();
        static RE_TRAIL_EP: OnceLock<Regex> = OnceLock::new();
        static RE_TRAIL_TAG: OnceLock<Regex> = OnceLock::new();

        let mut remaining = name.to_string();
        let mut sub_group = None;

        let re_group = RE_GROUP.get_or_init(|| Regex::new(r"^\[([^\]]+)\]").unwrap());
        if let Some(caps) = re_group.captures(&remaining) {
            sub_group = Some(caps.get(1).unwrap().as_str().to_string());
            remaining = remaining[caps.get(0).unwrap().end()..].trim().to_string();
        }

        if remaining.starts_with('[') {
            let parts: Vec<String> = remaining
                .split("][")
                .map(|p| p.trim_matches(|c| c == '[' || c == ']' || c == ' ').to_string())
                .filter(|p| !p.is_empty())
                .collect();
            if parts.len() >= 2 {
                if let Ok(ep) = parts[1].parse::<i32>() {
                    return ParsedFileName {
                        title: parts[0].clone(),
                        episode: Some(ep),
                        sub_group,
                        ..Default::default()
                    };
                }
            }
        }

        let re_trail_tag = RE_TRAIL_TAG.get_or_init(|| Regex::new(r"\s*\[[^\]]+\]$").unwrap());
        loop {
            let before = remaining.clone();
            remaining = re_trail_tag.replace(&remaining, "").to_string();
            if remaining == before {
                break;
            }
        }
        remaining = remaining.trim().to_string();

        let re_dash_ep = RE_DASH_EP.get_or_init(|| Regex::new(r"\s[-–]\s(\d{1,3})$").unwrap());
        if let Some(caps) = re_dash_ep.captures(&remaining) {
            let ep: i32 = caps.get(1).unwrap().as_str().parse().unwrap();
            let title = remaining[..caps.get(0).unwrap().start()].trim().to_string();
            return ParsedFileName {
                title,
                episode: Some(ep),
                sub_group,
                ..Default::default()
            };
        }

        let re_cn_ep = RE_CN_EP.get_or_init(|| Regex::new(r"第(\d{1,3})[話话集]").unwrap());
        if let Some(caps) = re_cn_ep.captures(&remaining) {
            let ep: i32 = caps.get(1).unwrap().as_str().parse().unwrap();
            let title = remaining[..caps.get(0).unwrap().start()].trim().to_string();
            return ParsedFileName {
                title,
                episode: Some(ep),
                sub_group,
                ..Default::default()
            };
        }

        let re_trail_ep = RE_TRAIL_EP.get_or_init(|| Regex::new(r"\s(\d{1,3})$").unwrap());
        if let Some(caps) = re_trail_ep.captures(&remaining) {
            let ep: i32 = caps.get(1).unwrap().as_str().parse().unwrap();
            let title = remaining[..caps.get(0).unwrap().start()].trim().to_string();
            return ParsedFileName {
                title,
                episode: Some(ep),
                sub_group,
                ..Default::default()
            };
        }

        ParsedFileName {
            title: remaining,
            sub_group,
            ..Default::default()
        }
    }

    fn parse_standard(name: &str) -> ParsedFileName {
        static RE_SE: OnceLock<Regex> = OnceLock::new();
        static RE_YEAR: OnceLock<Regex> = OnceLock::new();
        static RE_TRAIL_EP: OnceLock<Regex> = OnceLock::new();

        let mut s = name.replace('.', " ").replace('_', " ");

        let mut season = None;
        let mut episode = None;
        let re_se = RE_SE.get_or_init(|| Regex::new(r"(?i)S(\d{1,2})E(\d{1,3})").unwrap());
        if let Some(caps) = re_se.captures(&s) {
            season = caps.get(1).and_then(|m| m.as_str().parse().ok());
            episode = caps.get(2).and_then(|m| m.as_str().parse().ok());
            s = re_se.replace(&s, " ").to_string();
        }

        let mut year = None;
        let re_year = RE_YEAR.get_or_init(|| Regex::new(r"\(?((?:19|20)\d{2})\)?").unwrap());
        if let Some(caps) = re_year.captures(&s) {
            year = caps.get(1).and_then(|m| m.as_str().parse().ok());
            s = re_year.replace(&s, " ").to_string();
        }

        let mut tokens: Vec<&str> = Self::NOISE_TOKENS.to_vec();
        tokens.sort_by_key(|t| std::cmp::Reverse(t.len()));
        for token in tokens {
            let pattern = format!(r"(?i)\b{}\b", regex::escape(token));
            if let Ok(re) = Regex::new(&pattern) {
                s = re.replace_all(&s, " ").to_string();
            }
        }

        let title = s
            .split_whitespace()
            .filter(|t| !t.is_empty() && !is_residual_noise_token(t))
            .collect::<Vec<_>>()
            .join(" ");

        if season.is_none() {
            if let Some((base, season_num)) = Self::extract_season_suffix(&title) {
                return ParsedFileName {
                    title: base,
                    year,
                    season: Some(season_num),
                    episode,
                    sub_group: None,
                };
            }
        }

        if season.is_none() && episode.is_none() {
            let re_trail_ep = RE_TRAIL_EP.get_or_init(|| Regex::new(r"\s(\d{1,3})$").unwrap());
            if let Some(caps) = re_trail_ep.captures(&title) {
                let ep: i32 = caps.get(1).unwrap().as_str().parse().unwrap();
                let trimmed = title[..caps.get(0).unwrap().start()].to_string();
                return ParsedFileName {
                    title: trimmed,
                    year,
                    season,
                    episode: Some(ep),
                    sub_group: None,
                };
            }
        }

        ParsedFileName {
            title,
            year,
            season,
            episode,
            sub_group: None,
        }
    }

    fn drop_extension(filename: &str) -> String {
        let path = std::path::Path::new(filename);
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if Self::MEDIA_EXTENSIONS.contains(&ext.as_str()) {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(filename)
                .to_string()
        } else {
            path.file_name()
                .and_then(|s| s.to_str())
                .unwrap_or(filename)
                .to_string()
        }
    }
}

fn is_residual_noise_token(token: &str) -> bool {
    let normalized = token.to_ascii_uppercase();
    normalized == "HD" || token == "全集"
}

fn chinese_numeral(s: &str) -> Option<i32> {
    let map = |c: char| -> Option<i32> {
        match c {
            '一' => Some(1),
            '二' => Some(2),
            '三' => Some(3),
            '四' => Some(4),
            '五' => Some(5),
            '六' => Some(6),
            '七' => Some(7),
            '八' => Some(8),
            '九' => Some(9),
            '十' => Some(10),
            _ => None,
        }
    };
    if s == "十" {
        return Some(10);
    }
    if s.chars().count() == 1 {
        return map(s.chars().next()?);
    }
    if let Some(idx) = s.find('十') {
        let before = &s[..idx];
        let after = &s[idx + '十'.len_utf8()..];
        let tens = if before.is_empty() {
            1
        } else {
            map(before.chars().next()?)?
        };
        let ones = if after.is_empty() {
            0
        } else {
            map(after.chars().next()?)?
        };
        return Some(tens * 10 + ones);
    }
    map(s.chars().next()?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_movie_with_year() {
        let p = FileNameParser::parse("Oppenheimer.2023.1080p.BluRay.mkv");
        assert_eq!(p.title, "Oppenheimer");
        assert_eq!(p.year, Some(2023));
    }

    #[test]
    fn parses_tv_se() {
        let p = FileNameParser::parse("Show.Name.S02E05.mkv");
        assert_eq!(p.season, Some(2));
        assert_eq!(p.episode, Some(5));
    }

    #[test]
    fn parses_anime_dash_episode() {
        let p = FileNameParser::parse("[SubGroup] Title - 28.mkv");
        assert_eq!(p.sub_group.as_deref(), Some("SubGroup"));
        assert_eq!(p.title, "Title");
        assert_eq!(p.episode, Some(28));
    }

    #[test]
    fn extracts_cn_season_suffix() {
        let (base, season) = FileNameParser::extract_season_suffix("IT狂人 第 1 季").unwrap();
        assert_eq!(base, "IT狂人");
        assert_eq!(season, 1);
    }
}
