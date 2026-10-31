use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TuningSystem {
    pub name: String,
    pub description: String,
    pub degrees_cents: Vec<f64>,
    pub base_freq_hz: f64,
    pub base_midi_note: u8,
}

impl Default for TuningSystem {
    fn default() -> Self {
        Self::equal_temperament_12()
    }
}

impl TuningSystem {
    pub fn equal_temperament_12() -> Self {
        Self {
            name: "12-TET".to_string(),
            description: "Standard 12-Tone Equal Temperament".to_string(),
            degrees_cents: (0..12).map(|i| i as f64 * 100.0).collect(),
            base_freq_hz: 440.0,
            base_midi_note: 69,
        }
    }

    pub fn from_scala(text: &str) -> Result<Self, String> {
        let mut lines = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.starts_with('!') && !l.is_empty());
        let description = lines.next().unwrap_or("Custom Tuning").to_string();
        let count_str = lines
            .next()
            .ok_or_else(|| "Missing scale degree count".to_string())?;
        let count: usize = count_str
            .parse()
            .map_err(|e| format!("Invalid count: {}", e))?;

        let mut degrees = Vec::with_capacity(count + 1);
        degrees.push(0.0);

        for line in lines.take(count) {
            let token = line.split_whitespace().next().unwrap_or("");
            if token.contains('.') {
                let cents: f64 = token.parse().map_err(|e| format!("Invalid cents: {}", e))?;
                degrees.push(cents);
            } else if token.contains('/') {
                let parts: Vec<&str> = token.split('/').collect();
                if parts.len() == 2 {
                    let num: f64 = parts[0]
                        .parse()
                        .map_err(|e| format!("Invalid numerator: {}", e))?;
                    let den: f64 = parts[1]
                        .parse()
                        .map_err(|e| format!("Invalid denominator: {}", e))?;
                    if den > 0.0 {
                        let cents = 1200.0 * (num / den).log2();
                        degrees.push(cents);
                    }
                }
            } else if let Ok(val) = token.parse::<f64>() {
                degrees.push(val);
            }
        }

        Ok(Self {
            name: description.clone(),
            description,
            degrees_cents: degrees,
            base_freq_hz: 440.0,
            base_midi_note: 69,
        })
    }

    pub fn note_to_freq(&self, midi_note: u8) -> f64 {
        if self.degrees_cents.is_empty() {
            return 440.0 * 2.0f64.powf((midi_note as f64 - 69.0) / 12.0);
        }
        let (count, period_cents) = if self.degrees_cents.len() > 1
            && self.degrees_cents[0] == 0.0
            && self.degrees_cents.last().copied().unwrap_or(0.0) >= 100.0
            && self.degrees_cents.len() != 12
        {
            let cnt = self.degrees_cents.len() - 1;
            (cnt, self.degrees_cents[cnt])
        } else {
            (self.degrees_cents.len(), 1200.0)
        };
        let delta = midi_note as i32 - self.base_midi_note as i32;
        let octave = delta.div_euclid(count as i32);
        let deg = delta.rem_euclid(count as i32) as usize;
        let cents = octave as f64 * period_cents + self.degrees_cents[deg];
        let freq = self.base_freq_hz * 2.0f64.powf(cents / 1200.0);
        freq.clamp(20.0, 20000.0)
    }
}
