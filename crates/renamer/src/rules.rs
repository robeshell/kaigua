//! Independent Renamer rule engine (RENAME-R-01…08).
//! Aligned to ScrapeX RenamerKit `RenameRule` / `RulePipeline`.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum AnyRenameRule {
    TextReplace(TextReplace),
    RegexReplace(RegexReplace),
    InsertText(InsertText),
    DeleteRange(DeleteRange),
    CaseConversion(CaseConversion),
    AutoNumbering(AutoNumbering),
    StripBrackets(StripBrackets),
}

impl AnyRenameRule {
    pub fn id(&self) -> Uuid {
        match self {
            Self::TextReplace(r) => r.id,
            Self::RegexReplace(r) => r.id,
            Self::InsertText(r) => r.id,
            Self::DeleteRange(r) => r.id,
            Self::CaseConversion(r) => r.id,
            Self::AutoNumbering(r) => r.id,
            Self::StripBrackets(r) => r.id,
        }
    }

    pub fn apply(&self, filename: &str, index: usize) -> String {
        match self {
            Self::TextReplace(r) => r.apply(filename, index),
            Self::RegexReplace(r) => r.apply(filename, index),
            Self::InsertText(r) => r.apply(filename, index),
            Self::DeleteRange(r) => r.apply(filename, index),
            Self::CaseConversion(r) => r.apply(filename, index),
            Self::AutoNumbering(r) => r.apply(filename, index),
            Self::StripBrackets(r) => r.apply(filename, index),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TextReplace {
    pub id: Uuid,
    pub find: String,
    pub replacement: String,
}

impl TextReplace {
    pub fn new(find: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            find: find.into(),
            replacement: replacement.into(),
        }
    }

    pub fn apply(&self, filename: &str, _index: usize) -> String {
        filename.replace(&self.find, &self.replacement)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RegexReplace {
    pub id: Uuid,
    pub pattern: String,
    pub replacement: String,
}

impl RegexReplace {
    pub fn new(pattern: impl Into<String>, replacement: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            pattern: pattern.into(),
            replacement: replacement.into(),
        }
    }

    pub fn apply(&self, filename: &str, _index: usize) -> String {
        let Ok(re) = regex::Regex::new(&self.pattern) else {
            return filename.to_string();
        };
        // Swift NSRegularExpression `$1` templates map to Rust `${1}` / `$1`.
        re.replace_all(filename, self.replacement.as_str())
            .into_owned()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct InsertText {
    pub id: Uuid,
    pub text: String,
    /// 0 = beginning; negative = from end.
    pub position: i32,
}

impl InsertText {
    pub fn new(text: impl Into<String>, position: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            text: text.into(),
            position,
        }
    }

    pub fn apply(&self, filename: &str, _index: usize) -> String {
        let len = filename.chars().count() as i32;
        let idx = if self.position >= 0 {
            self.position.min(len).max(0) as usize
        } else {
            (len + self.position).max(0) as usize
        };
        let byte = filename
            .char_indices()
            .nth(idx)
            .map(|(i, _)| i)
            .unwrap_or(filename.len());
        let mut out = String::with_capacity(filename.len() + self.text.len());
        out.push_str(&filename[..byte]);
        out.push_str(&self.text);
        out.push_str(&filename[byte..]);
        out
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteRange {
    pub id: Uuid,
    pub from: i32,
    pub length: i32,
}

impl DeleteRange {
    pub fn new(from: i32, length: i32) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            length,
        }
    }

    pub fn apply(&self, filename: &str, _index: usize) -> String {
        let chars: Vec<char> = filename.chars().collect();
        let n = chars.len() as i32;
        if self.from < 0 || self.from >= n || self.length <= 0 {
            return filename.to_string();
        }
        let start = self.from as usize;
        let end = ((self.from + self.length).min(n)) as usize;
        chars[..start]
            .iter()
            .chain(chars[end..].iter())
            .collect()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum CaseMode {
    Title,
    Lower,
    Upper,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct CaseConversion {
    pub id: Uuid,
    pub mode: CaseMode,
}

impl CaseConversion {
    pub fn new(mode: CaseMode) -> Self {
        Self {
            id: Uuid::new_v4(),
            mode,
        }
    }

    pub fn apply(&self, filename: &str, _index: usize) -> String {
        match self.mode {
            CaseMode::Lower => filename.to_lowercase(),
            CaseMode::Upper => filename.to_uppercase(),
            CaseMode::Title => filename
                .split(' ')
                .filter(|word| !word.is_empty())
                .map(|word| {
                    let mut chars = word.chars();
                    match chars.next() {
                        None => String::new(),
                        Some(first) => {
                            let mut out = first.to_uppercase().collect::<String>();
                            out.push_str(&chars.as_str().to_lowercase());
                            out
                        }
                    }
                })
                .collect::<Vec<_>>()
                .join(" "),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NumberPosition {
    Prefix,
    Suffix,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct AutoNumbering {
    pub id: Uuid,
    pub start_at: i32,
    pub padding: u32,
    pub position: NumberPosition,
    pub separator: String,
}

impl AutoNumbering {
    pub fn new(
        start_at: i32,
        padding: u32,
        position: NumberPosition,
        separator: impl Into<String>,
    ) -> Self {
        Self {
            id: Uuid::new_v4(),
            start_at,
            padding,
            position,
            separator: separator.into(),
        }
    }

    pub fn apply(&self, filename: &str, index: usize) -> String {
        let number = self.start_at + index as i32;
        let pad = self.padding as usize;
        let formatted = format!("{number:0pad$}");
        match self.position {
            NumberPosition::Prefix => format!("{}{}{}", formatted, self.separator, filename),
            NumberPosition::Suffix => format!("{}{}{}", filename, self.separator, formatted),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BracketType {
    Square,
    Round,
    Curly,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct StripBrackets {
    pub id: Uuid,
    pub bracket_types: Vec<BracketType>,
}

impl StripBrackets {
    pub fn new(bracket_types: Vec<BracketType>) -> Self {
        Self {
            id: Uuid::new_v4(),
            bracket_types,
        }
    }

    pub fn apply(&self, filename: &str, _index: usize) -> String {
        let mut result = filename.to_string();
        for bracket in &self.bracket_types {
            let pattern = match bracket {
                BracketType::Square => r"\[[^\]]*\]",
                BracketType::Round => r"\([^)]*\)",
                BracketType::Curly => r"\{[^}]*\}",
            };
            if let Ok(re) = regex::Regex::new(pattern) {
                result = re.replace_all(&result, "").into_owned();
            }
        }
        while result.contains("  ") {
            result = result.replace("  ", " ");
        }
        result.trim().to_string()
    }
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct RulePipeline {
    pub rules: Vec<AnyRenameRule>,
}

impl RulePipeline {
    pub fn new(rules: Vec<AnyRenameRule>) -> Self {
        Self { rules }
    }

    pub fn apply(&self, filename: &str, index: usize) -> String {
        self.rules
            .iter()
            .fold(filename.to_string(), |acc, rule| rule.apply(&acc, index))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_replace_basic() {
        let rule = TextReplace::new("WEBDL", "");
        assert_eq!(rule.apply("Movie.WEBDL.1080p", 0), "Movie..1080p");
    }

    #[test]
    fn text_replace_with_value() {
        let rule = TextReplace::new("old", "new");
        assert_eq!(rule.apply("the old file", 0), "the new file");
    }

    #[test]
    fn regex_replace_capture() {
        let rule = RegexReplace::new(r"S(\d+)E(\d+)", "第$1季第$2集");
        assert_eq!(rule.apply("Show S01E05", 0), "Show 第01季第05集");
    }

    #[test]
    fn regex_replace_invalid_pattern() {
        let rule = RegexReplace::new("[invalid", "x");
        assert_eq!(rule.apply("test", 0), "test");
    }

    #[test]
    fn insert_text_at_beginning() {
        let rule = InsertText::new("PREFIX_", 0);
        assert_eq!(rule.apply("file", 0), "PREFIX_file");
    }

    #[test]
    fn insert_text_at_middle() {
        let rule = InsertText::new("-INSERT-", 4);
        assert_eq!(rule.apply("testfile", 0), "test-INSERT-file");
    }

    #[test]
    fn insert_text_from_end() {
        let rule = InsertText::new("_END", -3);
        assert_eq!(rule.apply("abcdef", 0), "abc_ENDdef");
    }

    #[test]
    fn delete_range_basic() {
        let rule = DeleteRange::new(2, 3);
        assert_eq!(rule.apply("abcdef", 0), "abf");
    }

    #[test]
    fn delete_range_out_of_bounds() {
        let rule = DeleteRange::new(10, 5);
        assert_eq!(rule.apply("short", 0), "short");
    }

    #[test]
    fn delete_range_clamps() {
        let rule = DeleteRange::new(3, 100);
        assert_eq!(rule.apply("abcdef", 0), "abc");
    }

    #[test]
    fn case_conversion_lower() {
        let rule = CaseConversion::new(CaseMode::Lower);
        assert_eq!(rule.apply("Hello WORLD", 0), "hello world");
    }

    #[test]
    fn case_conversion_upper() {
        let rule = CaseConversion::new(CaseMode::Upper);
        assert_eq!(rule.apply("Hello World", 0), "HELLO WORLD");
    }

    #[test]
    fn case_conversion_title() {
        let rule = CaseConversion::new(CaseMode::Title);
        assert_eq!(rule.apply("hello world test", 0), "Hello World Test");
    }

    #[test]
    fn auto_numbering_prefix() {
        let rule = AutoNumbering::new(1, 2, NumberPosition::Prefix, " ");
        assert_eq!(rule.apply("file", 0), "01 file");
        assert_eq!(rule.apply("file", 4), "05 file");
    }

    #[test]
    fn auto_numbering_suffix() {
        let rule = AutoNumbering::new(10, 3, NumberPosition::Suffix, "_");
        assert_eq!(rule.apply("file", 0), "file_010");
    }

    #[test]
    fn strip_brackets_square() {
        let rule = StripBrackets::new(vec![BracketType::Square]);
        assert_eq!(rule.apply("[字幕组] Title [1080p]", 0), "Title");
    }

    #[test]
    fn strip_brackets_round() {
        let rule = StripBrackets::new(vec![BracketType::Round]);
        assert_eq!(rule.apply("Title (2023) (BluRay)", 0), "Title");
    }

    #[test]
    fn strip_brackets_both() {
        let rule = StripBrackets::new(vec![BracketType::Square, BracketType::Round]);
        assert_eq!(rule.apply("[Sub] Title (2023)", 0), "Title");
    }

    #[test]
    fn pipeline_chain() {
        let pipeline = RulePipeline::new(vec![
            AnyRenameRule::StripBrackets(StripBrackets::new(vec![BracketType::Square])),
            AnyRenameRule::TextReplace(TextReplace::new("1080p", "")),
            AnyRenameRule::CaseConversion(CaseConversion::new(CaseMode::Title)),
        ]);
        assert_eq!(pipeline.apply("[Sub] my show 1080p", 0), "My Show");
    }

    #[test]
    fn pipeline_serde_roundtrip() {
        let pipeline = RulePipeline::new(vec![
            AnyRenameRule::TextReplace(TextReplace::new("a", "b")),
            AnyRenameRule::AutoNumbering(AutoNumbering::new(
                1,
                3,
                NumberPosition::Prefix,
                " ",
            )),
        ]);
        let json = serde_json::to_string(&pipeline).unwrap();
        let decoded: RulePipeline = serde_json::from_str(&json).unwrap();
        assert_eq!(decoded.rules.len(), 2);
        assert_eq!(decoded.apply("apple", 0), "001 bpple");
    }
}
