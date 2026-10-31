//! Parameter identities and values for devices (M03 item 7).
//!
//! A device kind has a table of parameters. A `ParamId` is a parameter's position in its kind's
//! table, so tables only grow at the end. Values are stored by id; the names of M01 stay the
//! adapter until the engine moves to ids (M05).

use serde::{Deserialize, Serialize};

/// Identifies a parameter within its device kind's table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ParamId(pub u16);

/// One parameter of a device kind: its name, its default, and how a change reaches the audio. A
/// parameter without a default has no value until it is set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ParamSpec {
    pub name: &'static str,
    pub default: Option<f32>,
    /// Milliseconds over which the engine ramps a change to this parameter. Zero applies the change
    /// at its sample offset with no ramp, which switches and enums need.
    pub smoothing_ms: f32,
}

/// The id of a parameter by name, if the table has it.
pub fn param_id(table: &[ParamSpec], name: &str) -> Option<ParamId> {
    table
        .iter()
        .position(|spec| spec.name == name)
        .map(|index| ParamId(index as u16))
}

/// Parameter values of one device, held sorted by id with at most one value per id.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ParamValues {
    values: Vec<(ParamId, f32)>,
}

impl ParamValues {
    /// The values a device starts with: one for each parameter that has a default.
    pub fn from_defaults(table: &[ParamSpec]) -> Self {
        let values = table
            .iter()
            .enumerate()
            .filter_map(|(index, spec)| {
                spec.default.map(|default| (ParamId(index as u16), default))
            })
            .collect();
        Self { values }
    }

    pub fn get(&self, id: ParamId) -> Option<f32> {
        self.values
            .binary_search_by_key(&id, |&(key, _)| key)
            .ok()
            .map(|index| self.values[index].1)
    }

    pub fn set(&mut self, id: ParamId, value: f32) {
        match self.values.binary_search_by_key(&id, |&(key, _)| key) {
            Ok(index) => self.values[index].1 = value,
            Err(index) => self.values.insert(index, (id, value)),
        }
    }

    /// Removes the value of a parameter, which then reads as unset.
    pub fn remove(&mut self, id: ParamId) {
        if let Ok(index) = self.values.binary_search_by_key(&id, |&(key, _)| key) {
            self.values.remove(index);
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (ParamId, f32)> + '_ {
        self.values.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TABLE: &[ParamSpec] = &[
        ParamSpec {
            name: "gain_db",
            default: Some(0.0),
            smoothing_ms: 10.0,
        },
        ParamSpec {
            name: "send",
            default: None,
            smoothing_ms: 10.0,
        },
        ParamSpec {
            name: "width",
            default: Some(1.0),
            smoothing_ms: 10.0,
        },
    ];

    #[test]
    fn defaults_skip_parameters_without_one() {
        let values = ParamValues::from_defaults(TABLE);
        assert_eq!(values.get(ParamId(0)), Some(0.0));
        assert_eq!(values.get(ParamId(1)), None);
        assert_eq!(values.get(ParamId(2)), Some(1.0));
    }

    #[test]
    fn set_keeps_ids_sorted_and_unique() {
        let mut values = ParamValues::default();
        values.set(ParamId(5), 0.5);
        values.set(ParamId(1), 0.1);
        values.set(ParamId(5), 0.7);
        let ids: Vec<_> = values.iter().collect();
        assert_eq!(ids, vec![(ParamId(1), 0.1), (ParamId(5), 0.7)]);
    }

    #[test]
    fn remove_clears_one_value_and_ignores_an_unset_one() {
        let mut values = ParamValues::from_defaults(TABLE);
        values.remove(ParamId(0));
        values.remove(ParamId(1));
        assert_eq!(values.get(ParamId(0)), None);
        assert_eq!(values.get(ParamId(2)), Some(1.0));
    }

    #[test]
    fn names_map_to_positions_in_the_table() {
        assert_eq!(param_id(TABLE, "width"), Some(ParamId(2)));
        assert_eq!(param_id(TABLE, "missing"), None);
    }
}
