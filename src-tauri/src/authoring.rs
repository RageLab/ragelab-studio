use std::{path::PathBuf, sync::Mutex};

use ragelab_engine::{
    workspace_scene_manifest_from_ymap_with_sources,
    workspace_scene_render_package_from_ymap_bytes_with_game_index, RenderPackage,
    RenderPackageOptions, SceneAssetPreviewSources, SceneManifestReport, YmapAuthoringReport,
    YmapAuthoringSaveResult, YmapAuthoringSession, YmapEditCommand, YmapMetaHash, YmapQuat,
    YmapVec3,
};
use serde::{Deserialize, Serialize};

use super::{core_error, native_viewport, validate_workspace_scene_context, WorkspaceSceneRequest};

#[derive(Default)]
pub(crate) struct AuthoringState {
    active: Mutex<Option<ActiveAuthoringSession>>,
}

struct ActiveAuthoringSession {
    request: WorkspaceSceneRequest,
    session: YmapAuthoringSession,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub(crate) enum StudioYmapEditCommand {
    SetTransform {
        index: usize,
        position: [f32; 3],
        rotation: [f32; 4],
        scale_xy: Option<f32>,
        scale_z: Option<f32>,
    },
    SetProperties {
        index: usize,
        archetype_hash: Option<u32>,
        flags: Option<u32>,
        parent_index: Option<i32>,
        #[serde(default)]
        clear_parent: bool,
    },
    Delete {
        indices: Vec<usize>,
    },
    Duplicate {
        indices: Vec<usize>,
        translation: [f32; 3],
    },
    Create {
        template_index: usize,
        archetype_hash: u32,
        position: [f32; 3],
        rotation: [f32; 4],
        scale_xy: Option<f32>,
        scale_z: Option<f32>,
        flags: u32,
        parent_index: Option<i32>,
    },
}

impl StudioYmapEditCommand {
    fn into_core(self) -> Result<YmapEditCommand, String> {
        Ok(match self {
            Self::SetTransform {
                index,
                position,
                rotation,
                scale_xy,
                scale_z,
            } => YmapEditCommand::SetTransform {
                index,
                position: vec3(position),
                rotation: quat(rotation),
                scale_xy,
                scale_z,
            },
            Self::SetProperties {
                index,
                archetype_hash,
                flags,
                parent_index,
                clear_parent,
            } => {
                if clear_parent && parent_index.is_some() {
                    return Err(
                        "YMAP property edit cannot set parentIndex and clearParent together".into(),
                    );
                }
                let parent_index = if clear_parent {
                    Some(None)
                } else {
                    parent_index.map(Some)
                };
                YmapEditCommand::SetProperties {
                    index,
                    archetype_name: archetype_hash.map(YmapMetaHash),
                    flags,
                    parent_index,
                }
            }
            Self::Delete { indices } => YmapEditCommand::Delete { indices },
            Self::Duplicate {
                indices,
                translation,
            } => YmapEditCommand::Duplicate {
                indices,
                translation: vec3(translation),
            },
            Self::Create {
                template_index,
                archetype_hash,
                position,
                rotation,
                scale_xy,
                scale_z,
                flags,
                parent_index,
            } => YmapEditCommand::CreateFromTemplate {
                template_index,
                archetype_name: YmapMetaHash(archetype_hash),
                position: vec3(position),
                rotation: quat(rotation),
                scale_xy,
                scale_z,
                flags,
                parent_index,
            },
        })
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthoringApplyRequest {
    commands: Vec<StudioYmapEditCommand>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthoringSaveRequest {
    output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthoringEntitySnapshot {
    index: usize,
    archetype_hash: u32,
    position: [f32; 3],
    rotation: [f32; 4],
    scale_xy: Option<f32>,
    scale_z: Option<f32>,
    flags: u32,
    parent_index: Option<i32>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthoringSnapshot {
    report: YmapAuthoringReport,
    entities: Vec<AuthoringEntitySnapshot>,
    manifest: SceneManifestReport,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AuthoringSaveResponse {
    save: YmapAuthoringSaveResult,
    snapshot: AuthoringSnapshot,
}

#[tauri::command]
pub(crate) fn authoring_open(
    state: tauri::State<'_, AuthoringState>,
    request: WorkspaceSceneRequest,
) -> Result<AuthoringSnapshot, String> {
    let scene = validate_request(&request)?;
    let session = YmapAuthoringSession::open(&scene.ymap).map_err(core_error)?;
    let active = ActiveAuthoringSession { request, session };
    let snapshot = snapshot(&active)?;
    let mut slot = state
        .active
        .lock()
        .map_err(|_| "Authoring state lock is poisoned".to_string())?;
    *slot = Some(active);
    Ok(snapshot)
}

#[tauri::command]
pub(crate) fn authoring_snapshot(
    state: tauri::State<'_, AuthoringState>,
) -> Result<AuthoringSnapshot, String> {
    with_active(&state, snapshot)
}

#[tauri::command]
pub(crate) fn authoring_apply(
    state: tauri::State<'_, AuthoringState>,
    request: AuthoringApplyRequest,
) -> Result<AuthoringSnapshot, String> {
    if request.commands.is_empty() {
        return Err("Authoring transaction must contain at least one command".into());
    }
    let commands = request
        .commands
        .into_iter()
        .map(StudioYmapEditCommand::into_core)
        .collect::<Result<Vec<_>, _>>()?;
    with_active_mut(&state, |active| {
        active.session.apply(&commands).map_err(core_error)?;
        snapshot(active)
    })
}

#[tauri::command]
pub(crate) fn authoring_undo(
    state: tauri::State<'_, AuthoringState>,
) -> Result<AuthoringSnapshot, String> {
    with_active_mut(&state, |active| {
        active.session.undo().map_err(core_error)?;
        snapshot(active)
    })
}

#[tauri::command]
pub(crate) fn authoring_redo(
    state: tauri::State<'_, AuthoringState>,
) -> Result<AuthoringSnapshot, String> {
    with_active_mut(&state, |active| {
        active.session.redo().map_err(core_error)?;
        snapshot(active)
    })
}

#[tauri::command]
pub(crate) fn authoring_revert(
    state: tauri::State<'_, AuthoringState>,
) -> Result<AuthoringSnapshot, String> {
    with_active_mut(&state, |active| {
        active.session.revert().map_err(core_error)?;
        snapshot(active)
    })
}

#[tauri::command]
pub(crate) fn authoring_save_as(
    state: tauri::State<'_, AuthoringState>,
    request: AuthoringSaveRequest,
) -> Result<AuthoringSaveResponse, String> {
    let output = PathBuf::from(request.output);
    with_active_mut(&state, |active| {
        let save = active.session.save_as(&output).map_err(core_error)?;
        let snapshot = snapshot(active)?;
        Ok(AuthoringSaveResponse { save, snapshot })
    })
}

#[tauri::command]
pub(crate) fn authoring_preview(
    authoring: tauri::State<'_, AuthoringState>,
    viewport: tauri::State<'_, native_viewport::NativeViewportState>,
) -> Result<native_viewport::NativeViewportReport, String> {
    let package = current_render_package(&authoring)?;
    native_viewport::update_render_package(&viewport, package)
}

#[tauri::command]
pub(crate) fn authoring_preview_world_overlay(
    authoring: tauri::State<'_, AuthoringState>,
    viewport: tauri::State<'_, native_viewport::NativeViewportState>,
) -> Result<native_viewport::NativeViewportReport, String> {
    let package = current_render_package(&authoring)?;
    native_viewport::set_world_overlay_packages(&viewport, vec![package])
}

fn current_render_package(
    authoring: &tauri::State<'_, AuthoringState>,
) -> Result<RenderPackage, String> {
    with_active(authoring, |active| {
        let scene = validate_request(&active.request)?;
        workspace_scene_render_package_from_ymap_bytes_with_game_index(
            &scene.workspace,
            &scene.ymap,
            active.session.bytes(),
            SceneAssetPreviewSources {
                fallback_roots: &scene.fallback_roots,
                rpf_mounts: &scene.rpf_mounts,
                game_index: scene.game_index.as_ref(),
            },
            scene.options,
            RenderPackageOptions::default(),
        )
        .map_err(core_error)
    })
}

#[tauri::command]
pub(crate) fn authoring_close(state: tauri::State<'_, AuthoringState>) -> Result<(), String> {
    let mut slot = state
        .active
        .lock()
        .map_err(|_| "Authoring state lock is poisoned".to_string())?;
    *slot = None;
    Ok(())
}

fn validate_request(
    request: &WorkspaceSceneRequest,
) -> Result<super::ValidatedWorkspaceScene, String> {
    validate_workspace_scene_context(
        &request.workspace,
        &request.ymap,
        &request.fallback_roots,
        &request.rpf_mounts,
        request.game_index.as_ref(),
        request.max_nodes,
    )
}

fn snapshot(active: &ActiveAuthoringSession) -> Result<AuthoringSnapshot, String> {
    let scene = validate_request(&active.request)?;
    let document = active.session.document().map_err(core_error)?;
    let manifest = workspace_scene_manifest_from_ymap_with_sources(
        &scene.workspace,
        &scene.ymap,
        &document,
        &scene.fallback_roots,
        &scene.rpf_mounts,
        scene.game_index.as_ref(),
        scene.options,
    )
    .map_err(core_error)?;
    let entities = document
        .entities
        .iter()
        .enumerate()
        .map(|(index, entity)| AuthoringEntitySnapshot {
            index,
            archetype_hash: entity.archetype_name.0,
            position: [entity.position.x, entity.position.y, entity.position.z],
            rotation: [
                entity.rotation.x,
                entity.rotation.y,
                entity.rotation.z,
                entity.rotation.w,
            ],
            scale_xy: entity.scale_xy,
            scale_z: entity.scale_z,
            flags: entity.flags,
            parent_index: entity.parent_index,
        })
        .collect();
    Ok(AuthoringSnapshot {
        report: active.session.report().map_err(core_error)?,
        entities,
        manifest: SceneManifestReport::from(&manifest),
    })
}

fn with_active<T>(
    state: &tauri::State<'_, AuthoringState>,
    callback: impl FnOnce(&ActiveAuthoringSession) -> Result<T, String>,
) -> Result<T, String> {
    let slot = state
        .active
        .lock()
        .map_err(|_| "Authoring state lock is poisoned".to_string())?;
    let active = slot
        .as_ref()
        .ok_or_else(|| "No YMAP authoring session is open".to_string())?;
    callback(active)
}

fn with_active_mut<T>(
    state: &tauri::State<'_, AuthoringState>,
    callback: impl FnOnce(&mut ActiveAuthoringSession) -> Result<T, String>,
) -> Result<T, String> {
    let mut slot = state
        .active
        .lock()
        .map_err(|_| "Authoring state lock is poisoned".to_string())?;
    let active = slot
        .as_mut()
        .ok_or_else(|| "No YMAP authoring session is open".to_string())?;
    callback(active)
}

fn vec3(value: [f32; 3]) -> YmapVec3 {
    YmapVec3 {
        x: value[0],
        y: value[1],
        z: value[2],
    }
}

fn quat(value: [f32; 4]) -> YmapQuat {
    YmapQuat {
        x: value[0],
        y: value[1],
        z: value[2],
        w: value[3],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn property_parent_adapter_rejects_conflicting_parent_intent() {
        let command = StudioYmapEditCommand::SetProperties {
            index: 0,
            archetype_hash: None,
            flags: None,
            parent_index: Some(1),
            clear_parent: true,
        };
        assert!(command.into_core().is_err());
    }

    #[test]
    fn group_duplicate_adapter_preserves_selection_and_translation() {
        let command = StudioYmapEditCommand::Duplicate {
            indices: vec![1, 3, 5],
            translation: [2.0, -4.0, 1.5],
        }
        .into_core()
        .expect("duplicate command");
        let YmapEditCommand::Duplicate {
            indices,
            translation,
        } = command
        else {
            panic!("expected duplicate command");
        };
        assert_eq!(indices, vec![1, 3, 5]);
        assert_eq!(
            translation,
            YmapVec3 {
                x: 2.0,
                y: -4.0,
                z: 1.5
            }
        );
    }
}
