use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CastMember {
    pub name: String,
    pub role: Option<String>,
    pub r#type: Option<String>,
    pub thumb_url: Option<String>,
    pub order: Option<i32>,
}

impl CastMember {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            role: None,
            r#type: None,
            thumb_url: None,
            order: None,
        }
    }
}
