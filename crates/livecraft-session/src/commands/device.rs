//! Device and parameter edits (M04 item 6). A device is named by its id. A parameter is named by
//! its name in the device kind's table, which is how the JSON and the CLI write it. The name is
//! resolved to an id each time the command runs.

use crate::command::{CommandError, Operation};
use crate::commands::finite;
use crate::registry::CommandRegistry;
use livecraft_model::automation::{AutomationLane, AutomationTarget};
use livecraft_model::device::{Device, DeviceType, param_table};
use livecraft_model::edit::EditError;
use livecraft_model::ids::{DeviceId, IdAllocator, TrackId};
use livecraft_model::model::Project;
use livecraft_model::param::{ParamId, param_id};
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<InsertDevice>();
    registry.register::<RemoveDevice>();
    registry.register::<MoveDevice>();
    registry.register::<SetDeviceEnabled>();
    registry.register::<SetDeviceParam>();
}

/// The device with this id on a track, for editing.
fn device_mut(
    project: &mut Project,
    track: TrackId,
    device: DeviceId,
) -> Result<&mut Device, CommandError> {
    let owner = project
        .track_mut(track)
        .ok_or(EditError::NoSuchTrack(track))?;
    owner
        .device_chain
        .iter_mut()
        .find(|known| known.id == device)
        .ok_or_else(|| EditError::NoSuchDevice { track, device }.into())
}

/// The id of a parameter of a device, from its name in the device kind's table.
fn param_of(device: &Device, name: &str) -> Result<ParamId, CommandError> {
    param_id(param_table(&device.device_type), name).ok_or_else(|| CommandError::NoSuchParam {
        device: device.id,
        name: name.to_string(),
    })
}

/// Adds a device to the end of a track's device chain.
#[derive(Debug, Serialize, Deserialize)]
pub struct InsertDevice {
    pub track: TrackId,
    pub device_type: DeviceType,
    pub name: String,
    #[serde(skip)]
    created: Option<DeviceId>,
}

impl InsertDevice {
    pub fn new(track: TrackId, device_type: DeviceType, name: impl Into<String>) -> Self {
        Self {
            track,
            device_type,
            name: name.into(),
            created: None,
        }
    }
}

impl Operation for InsertDevice {
    const NAME: &'static str = "device.insert";
    const LABEL: &'static str = "Insert Device";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let id = *self.created.get_or_insert_with(|| ids.device());
        let device = Device::new(id, &self.name, self.device_type.clone());
        project.add_device(self.track, device)?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(device) = self.created {
            let _ = project.remove_device(self.track, device);
        }
    }
}

/// A device that was removed, where it sat, and the automation that targeted it.
#[derive(Debug)]
struct RemovedDevice {
    track: TrackId,
    index: usize,
    device: Device,
    /// Each lane with its track and its position in that track's lanes, in position order.
    lanes: Vec<(TrackId, usize, AutomationLane)>,
}

fn targets_device(lane: &AutomationLane, device: DeviceId) -> bool {
    matches!(
        lane.target,
        AutomationTarget::DeviceParam { device_id, .. } if device_id == device
    )
}

/// Removes a device, with the automation that targets it. Undo puts both back where they were.
#[derive(Debug, Serialize, Deserialize)]
pub struct RemoveDevice {
    pub track: TrackId,
    pub device: DeviceId,
    #[serde(skip)]
    removed: Option<RemovedDevice>,
}

impl RemoveDevice {
    pub fn new(track: TrackId, device: DeviceId) -> Self {
        Self {
            track,
            device,
            removed: None,
        }
    }
}

impl Operation for RemoveDevice {
    const NAME: &'static str = "device.remove";
    const LABEL: &'static str = "Delete Device";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let owner = project
            .track(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        let index = owner
            .device_chain
            .iter()
            .position(|known| known.id == self.device)
            .ok_or(EditError::NoSuchDevice {
                track: self.track,
                device: self.device,
            })?;
        let device = owner.device_chain[index].clone();
        let mut lanes = Vec::new();
        for track in project.all_tracks() {
            for (position, lane) in track.automation_lanes.iter().enumerate() {
                if targets_device(lane, self.device) {
                    lanes.push((track.id, position, lane.clone()));
                }
            }
        }
        project.remove_device(self.track, self.device)?;
        self.removed = Some(RemovedDevice {
            track: self.track,
            index,
            device,
            lanes,
        });
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        let Some(RemovedDevice {
            track,
            index,
            device,
            lanes,
        }) = self.removed.take()
        else {
            return;
        };
        if let Some(owner) = project.track_mut(track) {
            let index = index.min(owner.device_chain.len());
            owner.device_chain.insert(index, device);
        }
        for (lane_track, position, lane) in lanes {
            if let Some(owner) = project.track_mut(lane_track) {
                let position = position.min(owner.automation_lanes.len());
                owner.automation_lanes.insert(position, lane);
            }
        }
    }
}

/// Moves a device to a new position in its track's device chain.
#[derive(Debug, Serialize, Deserialize)]
pub struct MoveDevice {
    pub track: TrackId,
    pub device: DeviceId,
    pub index: usize,
    #[serde(skip)]
    moved_from: Option<usize>,
}

impl MoveDevice {
    pub fn new(track: TrackId, device: DeviceId, index: usize) -> Self {
        Self {
            track,
            device,
            index,
            moved_from: None,
        }
    }
}

impl Operation for MoveDevice {
    const NAME: &'static str = "device.move";
    const LABEL: &'static str = "Move Device";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let owner = project
            .track_mut(self.track)
            .ok_or(EditError::NoSuchTrack(self.track))?;
        let chain = &mut owner.device_chain;
        let from = chain
            .iter()
            .position(|known| known.id == self.device)
            .ok_or(EditError::NoSuchDevice {
                track: self.track,
                device: self.device,
            })?;
        if self.index >= chain.len() {
            return Err(CommandError::OutOfRange {
                what: "device position",
                min: 0.0,
                max: chain.len().saturating_sub(1) as f64,
                value: self.index as f64,
            });
        }
        let device = chain.remove(from);
        chain.insert(self.index, device);
        self.moved_from = Some(from);
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        let Some(from) = self.moved_from.take() else {
            return;
        };
        if let Some(owner) = project.track_mut(self.track)
            && let Some(current) = owner
                .device_chain
                .iter()
                .position(|known| known.id == self.device)
        {
            let device = owner.device_chain.remove(current);
            let from = from.min(owner.device_chain.len());
            owner.device_chain.insert(from, device);
        }
    }
}

/// Turns a device on or off.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetDeviceEnabled {
    pub track: TrackId,
    pub device: DeviceId,
    pub enabled: bool,
    #[serde(skip)]
    previous: Option<bool>,
}

impl SetDeviceEnabled {
    pub fn new(track: TrackId, device: DeviceId, enabled: bool) -> Self {
        Self {
            track,
            device,
            enabled,
            previous: None,
        }
    }
}

impl Operation for SetDeviceEnabled {
    const NAME: &'static str = "device.set_enabled";
    const LABEL: &'static str = "Set Device Enabled";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let device = device_mut(project, self.track, self.device)?;
        self.previous = Some(std::mem::replace(&mut device.is_enabled, self.enabled));
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some(previous), Ok(device)) = (
            self.previous.take(),
            device_mut(project, self.track, self.device),
        ) {
            device.is_enabled = previous;
        }
    }
}

/// Sets one parameter of a device by its name. A knob drag merges into one undo step.
#[derive(Debug, Serialize, Deserialize)]
pub struct SetDeviceParam {
    pub track: TrackId,
    pub device: DeviceId,
    pub param: String,
    pub value: f32,
    /// The parameter and the value it held before, which is `None` when it had no value.
    #[serde(skip)]
    previous: Option<(ParamId, Option<f32>)>,
}

impl SetDeviceParam {
    pub fn new(track: TrackId, device: DeviceId, param: impl Into<String>, value: f32) -> Self {
        Self {
            track,
            device,
            param: param.into(),
            value,
            previous: None,
        }
    }
}

impl Operation for SetDeviceParam {
    const NAME: &'static str = "device.set_param";
    const LABEL: &'static str = "Set Device Parameter";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        finite("device parameter", f64::from(self.value))?;
        let device = device_mut(project, self.track, self.device)?;
        let id = param_of(device, &self.param)?;
        self.previous = Some((id, device.parameters.get(id)));
        device.parameters.set(id, self.value);
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let (Some((id, before)), Ok(device)) = (
            self.previous.take(),
            device_mut(project, self.track, self.device),
        ) {
            match before {
                Some(value) => device.parameters.set(id, value),
                None => device.parameters.remove(id),
            }
        }
    }

    fn absorb(&mut self, next: &Self) -> bool {
        if self.track == next.track && self.device == next.device && self.param == next.param {
            self.value = next.value;
            true
        } else {
            false
        }
    }
}
