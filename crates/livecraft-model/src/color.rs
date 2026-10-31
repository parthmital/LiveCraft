/// Standard Ableton Live 70-color palette (hex RGB values)
pub const ABLETON_PALETTE: &[&str] = &[
    "#ff4848", "#ff764c", "#ffa844", "#ffd03c", "#faff40", "#b9ff40", "#7bff40", "#40ff63",
    "#40ffa7", "#40ffec", "#40d2ff", "#408eff", "#4c5eff", "#814cff", "#b94cff", "#ff4ce6",
    "#ff4c99", "#d83838", "#d86038", "#d88a38", "#d8aa38", "#d0d838", "#93d838", "#61d838",
    "#38d855", "#38d88e", "#38d8c8", "#38aad8", "#3870d8", "#3d38d8", "#6638d8", "#9338d8",
    "#d838b9", "#d8387a", "#aa2828", "#aa4928", "#aa6c28", "#aa8528", "#a4aa28", "#74aa28",
    "#4daa28", "#28aa3f", "#28aa6f", "#28aa9c", "#2885aa", "#2857aa", "#2d28aa", "#4f28aa",
    "#7428aa", "#aa2892", "#aa285e", "#761818", "#763118", "#764918", "#765c18", "#717618",
    "#4f7618", "#337618", "#187629", "#18764c", "#18766c", "#185c76", "#183b76", "#1e1876",
    "#361876", "#4f1876", "#761864", "#76183f", "#ffffff", "#000000",
];

/// Parses a `#RRGGBB` or `#RRGGBBAA` color into RGBA bytes. Anything else is white. Non-ASCII
/// input is white too, because the digits are read by byte position.
pub fn parse_hex_color(hex: &str) -> [u8; 4] {
    let clean = hex.trim_start_matches('#');
    if !clean.is_ascii() {
        [255, 255, 255, 255]
    } else if clean.len() == 6 {
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
        [r, g, b, 255]
    } else if clean.len() == 8 {
        let r = u8::from_str_radix(&clean[0..2], 16).unwrap_or(255);
        let g = u8::from_str_radix(&clean[2..4], 16).unwrap_or(255);
        let b = u8::from_str_radix(&clean[4..6], 16).unwrap_or(255);
        let a = u8::from_str_radix(&clean[6..8], 16).unwrap_or(255);
        [r, g, b, a]
    } else {
        [255, 255, 255, 255]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hex_colors_parse_by_byte_and_non_ascii_input_is_white() {
        assert_eq!(parse_hex_color("#ff0000"), [255, 0, 0, 255]);
        assert_eq!(parse_hex_color("#00ff0080"), [0, 255, 0, 128]);
        assert_eq!(parse_hex_color("#ééé"), [255, 255, 255, 255]);
        assert_eq!(parse_hex_color("zz"), [255, 255, 255, 255]);
    }
}
