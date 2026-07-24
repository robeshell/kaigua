use media_core::MediaType;

use super::normalize_title;

/// Aligns to Swift `MatchScorer.relevance`.
pub fn relevance_score(
    query_title: &str,
    query_year: Option<i32>,
    candidate_title: &str,
    candidate_original_title: Option<&str>,
    candidate_year: Option<i32>,
    media_type: MediaType,
) -> f64 {
    let normalized_query = normalize_title(query_title);
    let normalized_title = normalize_title(candidate_title);
    let normalized_original = normalize_title(candidate_original_title.unwrap_or(""));

    let mut score = title_score(&normalized_query, &normalized_title)
        .max(title_score(&normalized_query, &normalized_original));

    score += year_bonus(query_year, candidate_year);

    match media_type {
        MediaType::Anime => {
            if !normalized_original.is_empty() && normalized_query == normalized_original {
                score += 0.08;
            }
        }
        MediaType::TvShow => {
            if normalized_title.contains("special") || normalized_original.contains("special") {
                score -= 0.1;
            }
        }
        MediaType::Movie => {}
    }

    score.clamp(0.0, 1.0)
}

fn title_score(query: &str, candidate: &str) -> f64 {
    if query.is_empty() || candidate.is_empty() {
        return 0.0;
    }
    if query == candidate {
        return 0.82;
    }
    if candidate.starts_with(query) || query.starts_with(candidate) {
        return 0.72;
    }
    if candidate.contains(query) || query.contains(candidate) {
        return 0.6;
    }

    let query_tokens: std::collections::HashSet<&str> = query.split_whitespace().collect();
    let candidate_tokens: std::collections::HashSet<&str> = candidate.split_whitespace().collect();
    let overlap = query_tokens.intersection(&candidate_tokens).count();
    let max_count = query_tokens.len().max(candidate_tokens.len());
    if max_count == 0 {
        return 0.0;
    }
    0.35 + (overlap as f64 / max_count as f64) * 0.25
}

fn year_bonus(query_year: Option<i32>, candidate_year: Option<i32>) -> f64 {
    match (query_year, candidate_year) {
        (Some(q), Some(c)) => match (q - c).abs() {
            0 => 0.18,
            1 => 0.08,
            _ => -0.12,
        },
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_title_scores_high() {
        let score = relevance_score(
            "Inception",
            Some(2010),
            "Inception",
            None,
            Some(2010),
            MediaType::Movie,
        );
        assert!(score > 0.9, "score={score}");
    }
}
