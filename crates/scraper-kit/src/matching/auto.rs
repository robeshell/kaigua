use media_core::MediaType;

use crate::types::SearchResult;

use super::normalize_title;

/// Aligns to Swift `AutoMatchEvaluator` — exact normalized title + year when present.
/// Does **not** use a confidence threshold.
pub fn auto_accepted_result(
    query_title: &str,
    query_year: Option<i32>,
    _media_type: MediaType,
    candidates: &[SearchResult],
) -> Option<SearchResult> {
    let mut sorted = candidates.to_vec();
    sorted.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    sorted.into_iter().find(|c| should_auto_accept(query_title, query_year, c))
}

fn should_auto_accept(query_title: &str, query_year: Option<i32>, candidate: &SearchResult) -> bool {
    let query = normalize_title(query_title);
    let title = normalize_title(&candidate.title);
    let original = normalize_title(candidate.original_title.as_deref().unwrap_or(""));
    let exact = query == title || (!original.is_empty() && query == original);
    if !exact {
        return false;
    }
    match query_year {
        None => true,
        Some(qy) => candidate.year == Some(qy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SearchResult;

    #[test]
    fn requires_exact_title_and_year() {
        let candidates = vec![SearchResult {
            source_id: "tmdb:1".into(),
            title: "Inception".into(),
            original_title: None,
            year: Some(2010),
            overview: None,
            poster_url: None,
            confidence: 0.9,
            media_type: MediaType::Movie,
        }];
        assert!(auto_accepted_result("Inception", Some(2010), MediaType::Movie, &candidates).is_some());
        assert!(auto_accepted_result("Inception", Some(2011), MediaType::Movie, &candidates).is_none());
        assert!(auto_accepted_result("Incept", Some(2010), MediaType::Movie, &candidates).is_none());
    }
}
