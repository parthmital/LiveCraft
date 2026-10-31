//! Scene edits (M04 item 7). A scene is named by its id. Its row in the Session View is where it
//! sits in the Set's scene list.

use crate::command::{CommandError, Operation};
use crate::registry::CommandRegistry;
use livecraft_model::edit::EditError;
use livecraft_model::ids::{IdAllocator, SceneId};
use livecraft_model::model::{Project, Scene};
use serde::{Deserialize, Serialize};

pub fn register(registry: &mut CommandRegistry) {
    registry.register::<CreateScene>();
    registry.register::<DeleteScene>();
    registry.register::<DuplicateScene>();
}

/// Adds a scene after the last row. The id comes from `ids` the first time, and a redo reuses it.
#[derive(Debug, Serialize, Deserialize)]
pub struct CreateScene {
    pub name: String,
    #[serde(skip)]
    created: Option<SceneId>,
}

impl CreateScene {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            created: None,
        }
    }
}

impl Operation for CreateScene {
    const NAME: &'static str = "scene.create";
    const LABEL: &'static str = "Create Scene";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let id = *self.created.get_or_insert_with(|| ids.scene());
        project.add_scene(Scene::new(id, &self.name))?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(id) = self.created {
            let _ = project.remove_scene(id);
        }
    }
}

/// A deleted scene and the row it was in.
#[derive(Debug)]
struct RemovedScene {
    row: usize,
    scene: Scene,
}

/// Deletes a scene. Undo puts it back in its row.
#[derive(Debug, Serialize, Deserialize)]
pub struct DeleteScene {
    pub scene: SceneId,
    #[serde(skip)]
    removed: Option<RemovedScene>,
}

impl DeleteScene {
    pub fn new(scene: SceneId) -> Self {
        Self {
            scene,
            removed: None,
        }
    }
}

impl Operation for DeleteScene {
    const NAME: &'static str = "scene.delete";
    const LABEL: &'static str = "Delete Scene";

    fn perform(
        &mut self,
        project: &mut Project,
        _ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let row = project
            .scene_row(self.scene)
            .ok_or(EditError::NoSuchScene(self.scene))?;
        let scene = project.scenes[row].clone();
        project.remove_scene(self.scene)?;
        self.removed = Some(RemovedScene { row, scene });
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(RemovedScene { row, scene }) = self.removed.take() {
            let _ = project.insert_scene(row, scene);
        }
    }
}

/// Copies a scene into the row after it. The copy has a new id and the same name and settings.
#[derive(Debug, Serialize, Deserialize)]
pub struct DuplicateScene {
    pub scene: SceneId,
    #[serde(skip)]
    created: Option<SceneId>,
}

impl DuplicateScene {
    pub fn new(scene: SceneId) -> Self {
        Self {
            scene,
            created: None,
        }
    }
}

impl Operation for DuplicateScene {
    const NAME: &'static str = "scene.duplicate";
    const LABEL: &'static str = "Duplicate Scene";

    fn perform(
        &mut self,
        project: &mut Project,
        ids: &mut IdAllocator,
    ) -> Result<(), CommandError> {
        let row = project
            .scene_row(self.scene)
            .ok_or(EditError::NoSuchScene(self.scene))?;
        let mut copy = project.scenes[row].clone();
        copy.id = *self.created.get_or_insert_with(|| ids.scene());
        project.insert_scene(row + 1, copy)?;
        Ok(())
    }

    fn revert(&mut self, project: &mut Project) {
        if let Some(id) = self.created {
            let _ = project.remove_scene(id);
        }
    }
}
