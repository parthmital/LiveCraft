use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AbletonTheme {
    pub name: String,
    pub colors: HashMap<String, String>, // Key -> Hex string (#RRGGBB or #RRGGBBAA)
}

impl Default for AbletonTheme {
    fn default() -> Self {
        // Authentic Ableton Live 12 "Default Dark Neutral Medium" theme colors
        let mut colors = HashMap::new();
        colors.insert("Desktop".to_string(), "#242424".to_string());
        colors.insert("SurfaceArea".to_string(), "#2b2b2b".to_string());
        colors.insert("SurfaceHighlight".to_string(), "#3b3b3b".to_string());
        colors.insert("DetailViewBackground".to_string(), "#323232".to_string());
        colors.insert("ControlBackground".to_string(), "#1e1e1e".to_string());
        colors.insert("ControlForeground".to_string(), "#b5b5b5".to_string());
        colors.insert("ControlDisabled".to_string(), "#555555".to_string());
        colors.insert("MeterBackground".to_string(), "#141414".to_string());
        colors.insert(
            "ViewCheckControlEnabledOn".to_string(),
            "#ffad56".to_string(),
        ); // Ableton Signature Amber
        colors.insert("ChosenRecord".to_string(), "#ff5559".to_string()); // Ableton Record Red
        colors.insert("ChosenPreListen".to_string(), "#3c6ab6".to_string()); // Ableton Solo/Cue Blue
        colors.insert("RangeDefault".to_string(), "#03c3d5".to_string()); // Ableton Cyan
        colors.insert("SelectionFrame".to_string(), "#ffad56".to_string());
        colors.insert("ScrollbarInnerHandle".to_string(), "#696969".to_string());
        colors.insert("ScrollbarLCDTrack".to_string(), "#363636".to_string());
        colors.insert("TextDisabled".to_string(), "#757575".to_string());
        colors.insert("VolumePeak".to_string(), "#38d855".to_string()); // Green meter
        colors.insert("VolumeWarn".to_string(), "#ffd03c".to_string()); // Yellow meter
        colors.insert("VolumeClip".to_string(), "#ff4848".to_string()); // Red clipping meter

        Self {
            name: "Default Dark Neutral Medium".to_string(),
            colors,
        }
    }
}

impl AbletonTheme {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        let content = fs::read_to_string(path.as_ref())?;
        let name = path
            .as_ref()
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Custom")
            .to_string();

        let mut colors = HashMap::new();
        // Parse simple XML tags: <Tag Value="#123456" />
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.starts_with('<') && trimmed.contains("Value=\"") {
                // e.g. <ControlForeground Value="#b5b5b5" />
                if let Some(tag_end) = trimmed.find(' ') {
                    let tag = &trimmed[1..tag_end];
                    if let Some(val_start) = trimmed.find("Value=\"") {
                        let after_val = &trimmed[val_start + 7..];
                        if let Some(val_end) = after_val.find('"') {
                            let hex_val = &after_val[..val_end];
                            colors.insert(tag.to_string(), hex_val.to_string());
                        }
                    }
                }
            }
        }

        Ok(Self { name, colors })
    }

    pub fn get_hex(&self, key: &str) -> &str {
        self.colors
            .get(key)
            .map(|s| s.as_str())
            .unwrap_or("#ffffff")
    }

    /// Parse hex string to RGBA [u8; 4]
    pub fn parse_color(&self, key: &str) -> [u8; 4] {
        let hex = self.get_hex(key);
        parse_hex_color(hex)
    }
}

/// The colour parser lives in the model, so the UI can use it without depending on this crate.
pub use livecraft_model::color::parse_hex_color;
