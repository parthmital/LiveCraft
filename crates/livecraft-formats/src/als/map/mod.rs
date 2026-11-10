//! Lossless bidirectional mappers between [`livecraft_model::model::Project`] and [`XmlTree`].

pub mod audio_clips;
pub mod automation;
pub mod clips;
pub mod devices;
pub mod notes;
pub mod routing;
pub mod scenes;
pub mod tracks;
pub mod transport;

use std::collections::{HashMap, HashSet};
use std::path::Path;
use std::sync::OnceLock;

use livecraft_model::ids::{ClipId, DeviceId, LaneId, SceneId, TrackId};
use livecraft_model::model::{Clip, Device, Project, Scene, Track};

use super::AlsError;
use super::read::{decompress_als_or_xml, read_xml_tree};
use super::tree::{XmlNode, XmlNodeId, XmlNodeKind, XmlTree};
use super::write::{XmlFormatMode, write_als_gz, write_xml_tree};

const EMPTY_SET_XML: &str = include_str!("../../../templates/empty_set.xml");

/// Identifies the model parameter targeted by an Ableton `AutomationTarget/@Id` (`PointeeId`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointeeTarget {
    Tempo,
    TimeSignature,
    TrackVolume(TrackId),
    TrackPan(TrackId),
    SendLevel(TrackId, usize),
    DeviceParam {
        track_id: TrackId,
        device_id: DeviceId,
        param_key: String,
    },
}

/// Allocates fresh Ableton XML `Id` / `PointeeId` / `NoteId` numbers above every ID in the document.
#[derive(Debug, Clone)]
pub struct AlsIdAllocator {
    next: u64,
    used: HashSet<u64>,
    original_next_pointee_id: Option<u64>,
    grew: bool,
}

impl Default for AlsIdAllocator {
    fn default() -> Self {
        Self::new(1)
    }
}

impl AlsIdAllocator {
    /// Creates an allocator starting at `next`.
    #[must_use]
    pub fn new(next: u64) -> Self {
        Self {
            next: next.max(1),
            used: HashSet::new(),
            original_next_pointee_id: None,
            grew: false,
        }
    }

    /// Records an existing numeric ID so newly allocated IDs never collide with it.
    pub fn reserve(&mut self, id: u64) {
        self.used.insert(id);
        if id >= self.next && id < u64::MAX {
            self.next = id + 1;
        }
    }

    /// Records the `NextPointeeId/@Value` read from the input document.
    pub fn seed_next_pointee_id(&mut self, value: u64) {
        self.original_next_pointee_id = Some(value);
        if value > self.next && value < u64::MAX {
            self.next = value;
        }
    }

    /// Allocates a fresh unique numeric ID above all previously observed IDs.
    pub fn alloc(&mut self) -> u64 {
        if self.next == 0 {
            self.next = 1;
        }
        while self.used.contains(&self.next) {
            self.next = if self.next == u64::MAX {
                1
            } else {
                self.next + 1
            };
        }
        let id = self.next;
        self.used.insert(id);
        self.next = if self.next < u64::MAX {
            self.next + 1
        } else {
            1
        };
        self.grew = true;
        id
    }

    /// Returns the next available ID (`NextPointeeId` candidate).
    #[must_use]
    pub fn next_id(&self) -> u64 {
        self.next
    }

    /// Returns true if `NextPointeeId/@Value` needs to be written/updated in XML.
    #[must_use]
    pub fn needs_next_pointee_update(&self) -> bool {
        match self.original_next_pointee_id {
            Some(orig) => self.grew || self.next > orig,
            None => self.grew,
        }
    }
}

/// Maps Live Set model entity IDs back to their originating [`XmlNodeId`]s inside [`XmlTree`].
#[derive(Debug, Clone, Default)]
pub struct EntityMap {
    pub ableton: XmlNodeIdFallback,
    pub live_set: XmlNodeIdFallback,
    pub tracks: HashMap<TrackId, XmlNodeId>,
    pub main_track: Option<XmlNodeId>,
    pub main_track_had_id_attr: bool,
    pub scenes: HashMap<SceneId, XmlNodeId>,
    pub clip_slots: HashMap<(TrackId, usize), XmlNodeId>,
    pub clips: HashMap<ClipId, XmlNodeId>,
    pub devices: HashMap<DeviceId, XmlNodeId>,
    pub automation_lanes: HashMap<LaneId, XmlNodeId>,
    pub pointee_targets: HashMap<u64, PointeeTarget>,
    pub tempo_envelope_node: Option<XmlNodeId>,
    pub time_signature_envelope_node: Option<XmlNodeId>,
}

/// Wrapper for an [`XmlNodeId`] defaulted to root `0` before initialization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct XmlNodeIdFallback(pub XmlNodeId);

impl Default for XmlNodeIdFallback {
    fn default() -> Self {
        Self(XmlNodeId(0))
    }
}

/// Lossless `.als` document holding the original [`XmlTree`], the synced [`Project`], and node mappings.
#[derive(Debug, Clone)]
pub struct AlsDocument {
    tree: XmlTree,
    project: Project,
    entities: EntityMap,
    ids: AlsIdAllocator,
}

impl AlsDocument {
    /// Opens a gzip-compressed `.als` file or plain UTF-8 `.xml` file losslessly.
    pub fn open<P: AsRef<Path>>(path: P) -> Result<Self, AlsError> {
        let path_ref = path.as_ref();
        let xml = decompress_als_or_xml(path_ref)?;
        let stem = path_ref
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");
        Self::from_xml_named(&xml, stem)
    }

    /// Parses an `.als` XML string into a lossless [`AlsDocument`] named `"Untitled"`.
    pub fn from_xml(xml: &str) -> Result<Self, AlsError> {
        Self::from_xml_named(xml, "Untitled")
    }

    /// Parses an `.als` XML string into a lossless [`AlsDocument`] with the given project name.
    pub fn from_xml_named(xml: &str, project_name: &str) -> Result<Self, AlsError> {
        let tree = read_xml_tree(xml)?;
        Self::from_tree(tree, project_name)
    }

    /// Builds a new [`AlsDocument`] from a [`Project`] starting from LiveCraft's empty-set template.
    pub fn from_project(project: &Project) -> Result<Self, AlsError> {
        static EMPTY_TEMPLATE: OnceLock<Option<AlsDocument>> = OnceLock::new();
        let mut doc = if let Some(Some(cached)) = EMPTY_TEMPLATE.get() {
            cached.clone()
        } else {
            let parsed = Self::from_xml_named(EMPTY_SET_XML, "Untitled")?;
            let _ = EMPTY_TEMPLATE.set(Some(parsed.clone()));
            parsed
        };
        doc.project.name = project.name.clone();
        doc.apply_project(project)?;
        Ok(doc)
    }

    fn from_tree(tree: XmlTree, project_name: &str) -> Result<Self, AlsError> {
        let root = tree.root();
        let root_name = tree.node(root).and_then(XmlNode::element_name);
        if root_name != Some("Ableton") {
            return Err(AlsError::InvalidFormat(format!(
                "expected `<Ableton>` root element, got `{}`",
                root_name.unwrap_or("<non-element>")
            )));
        }
        let live_set = tree
            .child(root, "LiveSet")
            .ok_or_else(|| AlsError::InvalidFormat("missing `<LiveSet>` element".to_owned()))?;

        let mut ids = AlsIdAllocator::new(1);
        collect_all_xml_ids(&tree, &mut ids);
        if let Some(next_str) = child_value_str(&tree, live_set, "NextPointeeId")
            && let Ok(v) = next_str.parse::<u64>()
        {
            ids.seed_next_pointee_id(v);
        }

        let mut entities = EntityMap {
            ableton: XmlNodeIdFallback(root),
            live_set: XmlNodeIdFallback(live_set),
            ..EntityMap::default()
        };

        let main_track_node = tracks::find_main_track_node(&tree, live_set);
        let parsed_tracks =
            tracks::read_all_tracks(&tree, live_set, main_track_node, &mut entities, &mut ids)?;
        let parsed_globals = transport::read_globals(
            &tree,
            root,
            live_set,
            main_track_node,
            &mut entities,
            &mut ids,
        )?;

        let project = Project {
            schema_version: parsed_globals.schema_version,
            creator: parsed_globals.creator,
            name: project_name.to_owned(),
            transport: parsed_globals.transport,
            scale: parsed_globals.scale,
            tuning: parsed_globals.tuning,
            tracks: parsed_tracks.tracks,
            return_tracks: parsed_tracks.return_tracks,
            main_track: parsed_tracks.main_track,
            scenes: parsed_globals.scenes,
        };

        Ok(Self {
            tree,
            project,
            entities,
            ids,
        })
    }

    /// Returns a reference to the mapped [`Project`].
    #[must_use]
    pub fn project(&self) -> &Project {
        &self.project
    }

    /// Consumes the document and returns the mapped [`Project`].
    #[must_use]
    pub fn into_project(self) -> Project {
        self.project
    }

    /// Returns a reference to the underlying lossless [`XmlTree`].
    #[must_use]
    pub fn tree(&self) -> &XmlTree {
        &self.tree
    }

    /// Returns a mutable reference to the underlying lossless [`XmlTree`].
    pub(crate) fn tree_mut(&mut self) -> &mut XmlTree {
        &mut self.tree
    }

    /// Returns the entity-to-[`XmlNodeId`] mapping table.
    #[must_use]
    pub fn entity_map(&self) -> &EntityMap {
        &self.entities
    }

    /// Returns the next allocatable numeric ID (`NextPointeeId`).
    #[must_use]
    pub fn next_pointee_id(&self) -> u64 {
        self.ids.next_id()
    }

    /// Returns the [`XmlNodeId`] backing track `id`, if present in the tree.
    #[must_use]
    pub fn track_node(&self, id: TrackId) -> Option<XmlNodeId> {
        self.entities.tracks.get(&id).copied()
    }

    /// Returns the [`XmlNodeId`] backing the Main track, if present.
    #[must_use]
    pub fn main_track_node(&self) -> Option<XmlNodeId> {
        self.entities.main_track
    }

    /// Returns the [`XmlNodeId`] backing scene `id`, if present.
    #[must_use]
    pub fn scene_node(&self, id: SceneId) -> Option<XmlNodeId> {
        self.entities.scenes.get(&id).copied()
    }

    /// Returns the [`XmlNodeId`] backing clip `id`, if present.
    #[must_use]
    pub fn clip_node(&self, id: ClipId) -> Option<XmlNodeId> {
        self.entities.clips.get(&id).copied()
    }

    /// Returns the [`XmlNodeId`] backing device `id`, if present.
    #[must_use]
    pub fn device_node(&self, id: DeviceId) -> Option<XmlNodeId> {
        self.entities.devices.get(&id).copied()
    }

    /// Patches the underlying [`XmlTree`] in place to match `project` without touching unmodified nodes.
    pub fn apply_project(&mut self, project: &Project) -> Result<(), AlsError> {
        reserve_project_ids(project, &mut self.ids);
        let root = self.tree.root();
        let live_set = self.entities.live_set.0;
        let baseline = self.project.clone();

        tracks::patch_all_tracks(
            &mut self.tree,
            live_set,
            &baseline,
            project,
            &mut self.entities,
            &mut self.ids,
        )?;

        transport::patch_globals(
            &mut self.tree,
            root,
            live_set,
            &baseline,
            project,
            &mut self.entities,
            &mut self.ids,
        )?;

        if self.ids.needs_next_pointee_update()
            && let Some(np) = self.tree.child(live_set, "NextPointeeId")
        {
            let next_str = self.ids.next_id().to_string();
            if self.tree.value_attr(np) != Some(next_str.as_str()) {
                self.tree.set_value_attr(np, next_str);
            }
        }

        self.project = project.clone();
        Ok(())
    }

    /// Serializes the document to UTF-8 XML preserving original formatting (`LiveExact`).
    pub fn to_xml(&self) -> Result<String, AlsError> {
        write_xml_tree(&self.tree, XmlFormatMode::LiveExact)
    }

    /// Serializes the document to VCS-friendly canonical XML with sorted attributes.
    pub fn to_pretty_xml(&self) -> Result<String, AlsError> {
        write_xml_tree(&self.tree, XmlFormatMode::PrettyCanonical)
    }

    /// Writes the document as a gzip-compressed `.als` file at `path`.
    pub fn save_als<P: AsRef<Path>>(&self, path: P) -> Result<(), AlsError> {
        write_als_gz(&self.tree, path.as_ref())
    }
}

fn collect_all_xml_ids(tree: &XmlTree, ids: &mut AlsIdAllocator) {
    let mut stack = vec![tree.root()];
    while let Some(nid) = stack.pop() {
        if let Some(node) = tree.node(nid) {
            if let Some(attrs) = node.attrs() {
                for attr in attrs {
                    if (attr.name == "Id" || attr.name == "NoteId")
                        && let Ok(v) = attr.value.parse::<u64>()
                    {
                        ids.reserve(v);
                    }
                }
            }
            if node.element_name() == Some("PointeeId")
                && let Some(v_str) = tree.value_attr(nid)
                && let Ok(v) = v_str.parse::<u64>()
            {
                ids.reserve(v);
            }
            stack.extend(node.children().iter().copied());
        }
    }
}

fn reserve_project_ids(project: &Project, ids: &mut AlsIdAllocator) {
    for trk in project.all_tracks() {
        ids.reserve(trk.id.0);
        for dev in &trk.device_chain {
            ids.reserve(dev.id.0);
        }
        for lane in &trk.automation_lanes {
            ids.reserve(lane.id.0);
        }
        for slot in &trk.clip_slots {
            ids.reserve(slot.id);
            if let Some(ref clip) = slot.clip {
                ids.reserve(clip.id.0);
                for lane in &clip.automation_lanes {
                    ids.reserve(lane.id.0);
                }
            }
        }
        for arr in &trk.arrangement_clips {
            ids.reserve(arr.id.0);
            ids.reserve(arr.clip.id.0);
            for lane in &arr.clip.automation_lanes {
                ids.reserve(lane.id.0);
            }
        }
    }
    for scene in &project.scenes {
        ids.reserve(scene.id.0);
    }
}

/// Imports a cached template XML tree's root element into `target` without re-parsing XML.
pub(crate) fn instantiate_template_cached(
    target: &mut XmlTree,
    cache: &'static OnceLock<Option<XmlTree>>,
    template_xml: &str,
) -> Result<XmlNodeId, AlsError> {
    let tpl = if let Some(Some(cached)) = cache.get() {
        cached
    } else {
        let parsed = read_xml_tree(template_xml)?;
        let _ = cache.set(Some(parsed));
        match cache.get() {
            Some(Some(cached)) => cached,
            _ => {
                let fallback = read_xml_tree(template_xml)?;
                return target
                    .import_subtree(&fallback, fallback.root())
                    .ok_or_else(|| {
                        AlsError::InvalidFormat("failed to import XML template".to_owned())
                    });
            }
        }
    };
    target
        .import_subtree(tpl, tpl.root())
        .ok_or_else(|| AlsError::InvalidFormat("failed to import XML template".to_owned()))
}

/// Appends a new `<child_name Value="..." />` directly onto `parent` without scanning existing children.
pub(crate) fn append_value_child(
    tree: &mut XmlTree,
    parent: XmlNodeId,
    child_name: &str,
    value: impl Into<String>,
) -> XmlNodeId {
    let cid = tree.alloc_element_with(
        child_name,
        vec![super::tree::XmlAttr::new("Value", value)],
        Some(true),
    );
    tree.append_child(parent, cid);
    cid
}

#[must_use]
pub(crate) fn child_value_str<'a>(
    tree: &'a XmlTree,
    parent: XmlNodeId,
    child_name: &str,
) -> Option<&'a str> {
    tree.child(parent, child_name)
        .and_then(|cid| tree.value_attr(cid))
}

#[must_use]
pub(crate) fn parse_bool_str(s: &str) -> Option<bool> {
    match s {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => None,
    }
}

#[must_use]
pub(crate) fn parse_child_bool(
    tree: &XmlTree,
    parent: XmlNodeId,
    child_name: &str,
) -> Option<bool> {
    child_value_str(tree, parent, child_name).and_then(parse_bool_str)
}

#[must_use]
pub(crate) fn parse_child_f32(tree: &XmlTree, parent: XmlNodeId, child_name: &str) -> Option<f32> {
    child_value_str(tree, parent, child_name)
        .and_then(|s| s.parse::<f32>().ok())
        .filter(|v| v.is_finite())
}

#[must_use]
pub(crate) fn parse_child_f64(tree: &XmlTree, parent: XmlNodeId, child_name: &str) -> Option<f64> {
    child_value_str(tree, parent, child_name)
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|v| v.is_finite())
}

#[must_use]
pub(crate) fn parse_child_i64(tree: &XmlTree, parent: XmlNodeId, child_name: &str) -> Option<i64> {
    child_value_str(tree, parent, child_name).and_then(|s| s.parse::<i64>().ok())
}

#[must_use]
pub(crate) fn parse_child_u64(tree: &XmlTree, parent: XmlNodeId, child_name: &str) -> Option<u64> {
    child_value_str(tree, parent, child_name).and_then(|s| s.parse::<u64>().ok())
}

pub(crate) fn set_child_value(
    tree: &mut XmlTree,
    parent: XmlNodeId,
    child_name: &str,
    value: impl Into<String>,
) {
    if let Some(cid) = tree.ensure_child(parent, child_name) {
        tree.set_value_attr(cid, value);
    }
}

#[must_use]
pub(crate) fn manual_value_str(tree: &XmlTree, param_node: XmlNodeId) -> Option<&str> {
    child_value_str(tree, param_node, "Manual")
}

pub(crate) fn set_manual_value(
    tree: &mut XmlTree,
    param_node: XmlNodeId,
    value: impl Into<String>,
) {
    set_child_value(tree, param_node, "Manual", value);
}

#[must_use]
pub(crate) fn format_f32(v: f32) -> String {
    if v == 0.0 || !v.is_finite() {
        "0".to_owned()
    } else {
        v.to_string()
    }
}

#[must_use]
pub(crate) fn format_f64(v: f64) -> String {
    if v == 0.0 || !v.is_finite() {
        "0".to_owned()
    } else {
        v.to_string()
    }
}

#[must_use]
pub(crate) fn bool_str(b: bool) -> &'static str {
    if b { "true" } else { "false" }
}

/// Converts Ableton linear gain (`0.0..~2.0`) to LiveCraft dB (`-60.0` floor for `<= 0.000317`).
#[must_use]
pub(crate) fn linear_to_db(lin: f32) -> f32 {
    if !lin.is_finite() || lin <= 0.000_317 {
        -60.0
    } else {
        20.0 * lin.log10()
    }
}

/// Converts LiveCraft dB to Ableton linear gain (`0.0` for `<= -60.0`).
#[must_use]
pub(crate) fn db_to_linear(db: f32) -> f32 {
    if !db.is_finite() || db <= -60.0 {
        0.0
    } else {
        10.0_f32.powf(db / 20.0)
    }
}

#[must_use]
pub(crate) fn f32_eq(a: f32, b: f32) -> bool {
    (a.is_nan() && b.is_nan()) || a == b
}

#[must_use]
pub(crate) fn f64_eq(a: f64, b: f64) -> bool {
    (a.is_nan() && b.is_nan()) || a == b
}

/// Inserts or replaces direct child elements matching `predicate` in `parent` while keeping other children.
pub(crate) fn sync_ordered_children<F>(
    tree: &mut XmlTree,
    parent: XmlNodeId,
    is_managed: F,
    desired: Vec<XmlNodeId>,
) where
    F: Fn(&XmlNode) -> bool,
{
    let existing = tree.children(parent).to_vec();
    let current_managed: Vec<XmlNodeId> = existing
        .iter()
        .copied()
        .filter(|&cid| tree.node(cid).is_some_and(&is_managed))
        .collect();
    if current_managed == desired {
        return;
    }

    let mut result = Vec::with_capacity(existing.len() + desired.len());
    let mut inserted = false;
    for cid in existing {
        if tree.node(cid).is_some_and(&is_managed) {
            if !inserted {
                result.extend_from_slice(&desired);
                inserted = true;
            }
        } else {
            result.push(cid);
        }
    }
    if !inserted {
        result.extend_from_slice(&desired);
    }
    tree.replace_children(parent, result);
}

#[must_use]
pub(crate) fn scenes_equal(a: &Scene, b: &Scene) -> bool {
    a.id == b.id
        && a.name == b.name
        && a.color == b.color
        && match (a.tempo, b.tempo) {
            (None, None) => true,
            (Some(x), Some(y)) => f32_eq(x, y),
            _ => false,
        }
        && a.time_signature == b.time_signature
}

#[must_use]
pub(crate) fn devices_equal(a: &Device, b: &Device) -> bool {
    a.id == b.id
        && a.name == b.name
        && a.device_type == b.device_type
        && a.is_enabled == b.is_enabled
        && a.parameters == b.parameters
        && a.sample_refs == b.sample_refs
}

#[must_use]
pub(crate) fn clips_equal(a: &Clip, b: &Clip) -> bool {
    a.id == b.id
        && a.name == b.name
        && a.color == b.color
        && f64_eq(a.length_beats, b.length_beats)
        && f64_eq(a.loop_start_beat, b.loop_start_beat)
        && f64_eq(a.loop_end_beat, b.loop_end_beat)
        && a.is_looping == b.is_looping
        && f64_eq(a.start_marker_beat, b.start_marker_beat)
        && f64_eq(a.end_marker_beat, b.end_marker_beat)
        && a.automation_lanes == b.automation_lanes
        && a.launch == b.launch
        && a.follow == b.follow
        && payloads_equal(&a.payload, &b.payload)
}

#[must_use]
fn payloads_equal(
    a: &livecraft_model::model::ClipPayload,
    b: &livecraft_model::model::ClipPayload,
) -> bool {
    use livecraft_model::model::ClipPayload;
    match (a, b) {
        (ClipPayload::Midi(ma), ClipPayload::Midi(mb)) => {
            ma.notes.len() == mb.notes.len()
                && ma.notes.iter().zip(&mb.notes).all(|(x, y)| {
                    x.pitch == y.pitch
                        && f64_eq(x.start_beat, y.start_beat)
                        && f64_eq(x.duration_beats, y.duration_beats)
                        && f32_eq(x.velocity, y.velocity)
                        && f32_eq(x.release_velocity, y.release_velocity)
                        && f32_eq(x.probability, y.probability)
                        && f32_eq(x.velocity_deviation, y.velocity_deviation)
                        && x.mute == y.mute
                })
        }
        (ClipPayload::Audio(aa), ClipPayload::Audio(ab)) => {
            aa.file_path == ab.file_path
                && aa.relative_path == ab.relative_path
                && aa.sample_rate == ab.sample_rate
                && aa.channels == ab.channels
                && aa.warp_mode == ab.warp_mode
                && aa.is_warped == ab.is_warped
                && f64_eq(aa.original_bpm, ab.original_bpm)
                && f32_eq(aa.gain_db, ab.gain_db)
                && aa.pitch_cents == ab.pitch_cents
                && aa.ram_mode == ab.ram_mode
                && aa.warp_markers.len() == ab.warp_markers.len()
                && aa.warp_markers.iter().zip(&ab.warp_markers).all(|(x, y)| {
                    f64_eq(x.sample_time_sec, y.sample_time_sec) && f64_eq(x.beat_time, y.beat_time)
                })
        }
        _ => false,
    }
}

#[must_use]
pub(crate) fn tracks_equal(a: &Track, b: &Track) -> bool {
    a.id == b.id
        && a.name == b.name
        && a.track_type == b.track_type
        && a.parent == b.parent
        && a.routing == b.routing
        && a.color == b.color
        && f32_eq(a.volume_db, b.volume_db)
        && f32_eq(a.pan, b.pan)
        && a.is_muted == b.is_muted
        && a.is_soloed == b.is_soloed
        && a.is_armed == b.is_armed
        && a.monitoring == b.monitoring
        && a.sends.len() == b.sends.len()
        && a.sends.iter().zip(&b.sends).all(|(&x, &y)| f32_eq(x, y))
        && a.clip_slots.len() == b.clip_slots.len()
        && a.clip_slots.iter().zip(&b.clip_slots).all(|(x, y)| {
            x.id == y.id
                && x.has_stop_button == y.has_stop_button
                && match (&x.clip, &y.clip) {
                    (None, None) => true,
                    (Some(ca), Some(cb)) => clips_equal(ca, cb),
                    _ => false,
                }
        })
        && a.arrangement_clips.len() == b.arrangement_clips.len()
        && a.arrangement_clips
            .iter()
            .zip(&b.arrangement_clips)
            .all(|(x, y)| {
                x.id == y.id
                    && f64_eq(x.start_beat, y.start_beat)
                    && f64_eq(x.length_beats, y.length_beats)
                    && clips_equal(&x.clip, &y.clip)
            })
        && a.automation_lanes == b.automation_lanes
        && a.device_chain.len() == b.device_chain.len()
        && a.device_chain
            .iter()
            .zip(&b.device_chain)
            .all(|(x, y)| devices_equal(x, y))
}

#[must_use]
pub(crate) fn element_has_name(node: &XmlNode, names: &[&str]) -> bool {
    matches!(
        &node.kind,
        XmlNodeKind::Element { name, .. } if names.contains(&name.as_str())
    )
}
