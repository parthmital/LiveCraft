use crate::ids::SceneId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scene {
    pub id: SceneId,
    pub name: String,
    pub tempo: Option<f32>,
    pub time_signature: Option<(u8, u8)>,
    pub color: String,
}

impl Scene {
    pub fn new(id: SceneId, name: impl Into<String>) -> Self {
        Self {
            id,
            name: name.into(),
            tempo: None,
            time_signature: None,
            color: "#464646".to_string(),
        }
    }
}
