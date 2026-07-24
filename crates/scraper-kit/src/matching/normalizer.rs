use regex::Regex;
use std::sync::OnceLock;

/// Aligns to Swift `TitleNormalizer.normalize`.
pub fn normalize_title(title: &str) -> String {
    if title.is_empty() {
        return String::new();
    }
    let stripped = remove_wrapped_year_suffix(title);
    let lowered = stripped.to_lowercase();
    let replaced = lowered
        .replace('：', ":")
        .replace('－', "-")
        .replace('—', "-")
        .replace('–', "-")
        .replace('_', " ")
        .replace('-', " ");

    let filtered: String = replaced
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c.is_whitespace() {
                c
            } else {
                ' '
            }
        })
        .collect();

    filtered.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn remove_wrapped_year_suffix(title: &str) -> String {
    static PATTERNS: OnceLock<Vec<Regex>> = OnceLock::new();
    let patterns = PATTERNS.get_or_init(|| {
        [
            r"\s*\((19|20)\d{2}\)\s*$",
            r"\s*（(19|20)\d{2}）\s*$",
            r"\s*\[(19|20)\d{2}\]\s*$",
            r"\s*【(19|20)\d{2}】\s*$",
        ]
        .into_iter()
        .map(|p| Regex::new(p).expect("year suffix regex"))
        .collect()
    });
    let mut out = title.to_string();
    for re in patterns {
        out = re.replace(&out, "").into_owned();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_year_and_normalizes_separators() {
        assert_eq!(
            normalize_title("Love, Death & Robots (2019)"),
            "love death robots"
        );
        assert_eq!(
            normalize_title("Love - Death Robots"),
            "love death robots"
        );
    }
}
