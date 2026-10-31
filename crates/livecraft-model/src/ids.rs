//! Typed identities for Live Set entities.
//!
//! Commands, engine messages, and files refer to entities by these ids, never by
//! list position. An id is allocated once and never reused within a Set.

use serde::{Deserialize, Serialize};

macro_rules! define_id {
    ($(#[$doc:meta])* $name:ident) => {
        $(#[$doc])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
        pub struct $name(pub u64);

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                std::fmt::Display::fmt(&self.0, f)
            }
        }

        /// Wraps a number read from a file or written as a literal. New ids come from `IdAllocator`.
        impl From<u64> for $name {
            fn from(id: u64) -> Self {
                Self(id)
            }
        }
    };
}

define_id!(
    /// Identifies a track: audio, MIDI, group, return, or the Main track.
    TrackId
);
define_id!(
    /// Identifies a clip in a clip slot or in the arrangement.
    ClipId
);
define_id!(
    /// Identifies a scene (a row of the Session View).
    SceneId
);
define_id!(
    /// Identifies a device in a track's device chain.
    DeviceId
);
define_id!(
    /// Identifies an automation lane.
    LaneId
);

/// Hands out ids that increase by one and are never reused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdAllocator {
    next: u64,
}

impl Default for IdAllocator {
    fn default() -> Self {
        Self { next: 1 }
    }
}

impl IdAllocator {
    /// Creates an allocator whose first id is `1`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates an allocator whose next id is `next`, for seeding from a file.
    pub fn starting_at(next: u64) -> Self {
        Self { next }
    }

    fn take(&mut self) -> u64 {
        let id = self.next;
        self.next = self.next.saturating_add(1);
        id
    }

    pub fn track(&mut self) -> TrackId {
        TrackId(self.take())
    }

    pub fn clip(&mut self) -> ClipId {
        ClipId(self.take())
    }

    pub fn scene(&mut self) -> SceneId {
        SceneId(self.take())
    }

    pub fn device(&mut self) -> DeviceId {
        DeviceId(self.take())
    }

    pub fn lane(&mut self) -> LaneId {
        LaneId(self.take())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocator_counts_up_from_one_and_never_repeats() {
        let mut ids = IdAllocator::new();
        assert_eq!(ids.track(), TrackId(1));
        assert_eq!(ids.clip(), ClipId(2));
        assert_eq!(ids.track(), TrackId(3));
        assert_eq!(ids.scene(), SceneId(4));
        assert_eq!(ids.device(), DeviceId(5));
        assert_eq!(ids.lane(), LaneId(6));
    }

    #[test]
    fn allocator_resumes_from_a_seed() {
        let mut ids = IdAllocator::starting_at(1000);
        assert_eq!(ids.track(), TrackId(1000));
        assert_eq!(ids.track(), TrackId(1001));
    }

    #[test]
    fn ids_display_as_their_number() {
        assert_eq!(TrackId(7).to_string(), "7");
        assert_eq!(DeviceId(301).to_string(), "301");
    }

    #[test]
    fn ids_serialise_as_plain_numbers() {
        let json = serde_json::to_string(&TrackId(7)).expect("serialise id");
        assert_eq!(json, "7");
        let back: TrackId = serde_json::from_str("7").expect("deserialise id");
        assert_eq!(back, TrackId(7));
    }
}
