use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: u64,
    pub name: String,
    pub tempo: Option<f32>,
    pub time_signature: Option<(u8, u8)>,
    pub color: String,
}

impl Scene {
    pub fn new(id: u64, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            tempo: None,
            time_signature: None,
            color: "#464646".to_string(),
        }
    }
}
