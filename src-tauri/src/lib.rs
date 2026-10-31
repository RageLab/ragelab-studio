use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

use ragelab_engine::{
    apply_operation_document, asset_capabilities, decode_glb_geometry_bytes,
    default_glb_authoring_profile, discover_fivem_legacy, discover_gta_v_legacy, export_glb_to_ydr,
    export_ytd_texture_png, inspect_asset, inspect_glb_bytes, plan_operation_document,
    prepare_gta_rpf_index, prepare_gta_rpf_keys, preview_asset,
    validate_glb_authoring_profile_bytes, workspace_export_preflight_report,
    workspace_export_report, workspace_scene_asset_preview_with_game_index,
    workspace_scene_report_with_game_index, AssetCapabilitiesReport, AssetInspectionReport,
    AssetPreviewReport, CatalogRefs, DecodedGeometryReport, FiveMDiscoveryReport,
    GlbInspectionReport, GlbShaderTemplateFormat, GlbShaderTemplateInput, GlbYdrExportReport,
    GlbYdrExportRequest, GtaAuthoringProfile, GtaRpfIndexCacheReport, GtaRpfKeyCacheReport,
    GtaVDiscoveryReport, OperationApplyResult, OperationDocument, OperationPlan, PreviewOptions,
    ProfileValidationReport, SceneAssemblyOptions, SceneAssetPreviewSources, SceneGameIndexSource,
    SceneManifestReport, SceneRpfMount, SharedExportOptions, WorkspaceExportPreflightReport,
    WorkspaceExportReport, CORE_API_COMPATIBILITY_VERSION, CORE_DIRTY, CORE_REVISION,
    MAX_SCENE_NODE_LIMIT,
};
use serde::{Deserialize, Serialize};

mod authoring;
mod diagnostics;
mod native_viewport;

use authoring::{
    authoring_apply, authoring_close, authoring_open, authoring_preview,
    authoring_preview_world_overlay, authoring_redo, authoring_revert, authoring_save_as,
    authoring_snapshot, authoring_undo, AuthoringState,
};
use diagnostics::{diagnostics_record_frontend_error, diagnostics_summary};
use native_viewport::{
    debug_three_viewport_benchmark_complete, debug_three_viewport_benchmark_spec,
    native_viewport_create, native_viewport_create_dedicated, native_viewport_focus_node,
    native_viewport_focus_window, native_viewport_input, native_viewport_isolate_node,
    native_viewport_load_scene, native_viewport_open_dedicated_scene,
    native_viewport_open_dedicated_world, native_viewport_pick, native_viewport_select,
    native_viewport_set_gizmo, native_viewport_set_layer_visibility,
    native_viewport_set_node_visible, native_viewport_set_overlays, native_viewport_set_projection,
    native_viewport_set_rect, native_viewport_set_visible, native_viewport_show_all_nodes,
    native_viewport_shutdown, native_viewport_start_world_stream, native_viewport_stats,
    native_viewport_stop_world_stream, native_viewport_world_clear_overlays,
    native_viewport_world_search, native_viewport_world_set_workspace_overlay,
    native_viewport_world_stream_at, NativeViewportState,
};

#[derive(Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
struct StudioInfo {
    product: &'static str,
    version: &'static str,
    core_revision: &'static str,
    core_dirty: bool,
    core_api_compatibility: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepareGtaRpfKeysRequest {
    game_root: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PrepareGtaRpfIndexRequest {
    game_root: String,
    keys: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct YtdTexturePngExportRequest {
    path: String,
    texture_index: usize,
    output: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct YtdTexturePngExportReport {
    output: String,
    bytes_written: usize,
    source_unchanged: bool,
}

#[derive(Debug, Clone, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
struct GlbProfileValidateRequest {
    source: String,
    profile: String,
}

#[derive(Debug, Clone, Deserialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
struct GlbExportCommandRequest {
    source: String,
    output: String,
    profile: Option<String>,
    shader_template: Option<String>,
    shader_index: Option<usize>,
    #[serde(default)]
    overwrite: bool,
}

#[derive(Debug, Clone, Serialize, ts_rs::TS)]
#[serde(rename_all = "camelCase")]
struct StudioGlbExportReport {
    source: String,
    output: String,
    bytes_written: usize,
    engine: GlbYdrExportReport,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneRpfMountRequest {
    archive: String,
    #[serde(default)]
    nested: Vec<String>,
    keys: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SceneGameIndexRequest {
    game_root: String,
    index: String,
    keys: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSceneRequest {
    workspace: String,
    ymap: String,
    #[serde(default)]
    fallback_roots: Vec<String>,
    #[serde(default)]
    rpf_mounts: Vec<SceneRpfMountRequest>,
    game_index: Option<SceneGameIndexRequest>,
    max_nodes: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSceneAssetPreviewRequest {
    workspace: String,
    ymap: String,
    #[serde(default)]
    fallback_roots: Vec<String>,
    #[serde(default)]
    rpf_mounts: Vec<SceneRpfMountRequest>,
    game_index: Option<SceneGameIndexRequest>,
    max_nodes: Option<usize>,
    asset_ref: usize,
}

struct ValidatedWorkspaceScene {
    workspace: PathBuf,
    ymap: PathBuf,
    fallback_roots: Vec<PathBuf>,
    rpf_mounts: Vec<SceneRpfMount>,
    game_index: Option<SceneGameIndexSource>,
    options: SceneAssemblyOptions,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceExportPreflightRequest {
    workspace: String,
    maps: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceExportRequest {
    workspace: String,
    maps: Vec<String>,
    output: String,
    resource_name: String,
    allow_unresolved: bool,
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PreviewRequest {
    path: String,
    drawable_index: Option<usize>,
    max_primitives: Option<usize>,
    max_vertices: Option<usize>,
    max_indices: Option<usize>,
    max_shaders: Option<usize>,
    max_texture_references: Option<usize>,
    max_children: Option<usize>,
    max_materials: Option<usize>,
}

impl PreviewRequest {
    fn options(&self) -> PreviewOptions {
        let mut options = PreviewOptions {
            drawable_index: self.drawable_index,
            ..PreviewOptions::default()
        };

        if let Some(value) = self.max_primitives {
            options.max_primitives = value;
        }
        if let Some(value) = self.max_vertices {
            options.max_vertices = value;
        }
        if let Some(value) = self.max_indices {
            options.max_indices = value;
        }
        if let Some(value) = self.max_shaders {
            options.max_shaders = value;
        }
        if let Some(value) = self.max_texture_references {
            options.max_texture_references = value;
        }
        if let Some(value) = self.max_children {
            options.max_children = value;
        }
        if let Some(value) = self.max_materials {
            options.max_materials = value;
        }

        options
    }
}

#[tauri::command]
fn studio_info() -> StudioInfo {
    StudioInfo {
        product: "RageLab Studio",
        version: env!("CARGO_PKG_VERSION"),
        core_revision: CORE_REVISION,
        core_dirty: CORE_DIRTY,
        core_api_compatibility: CORE_API_COMPATIBILITY_VERSION,
    }
}

#[tauri::command]
fn core_discover_gta_legacy() -> GtaVDiscoveryReport {
    discover_gta_v_legacy()
}

#[tauri::command]
fn core_discover_fivem_legacy() -> FiveMDiscoveryReport {
    discover_fivem_legacy()
}

#[tauri::command]
fn core_prepare_gta_rpf_keys(
    request: PrepareGtaRpfKeysRequest,
) -> Result<GtaRpfKeyCacheReport, String> {
    let game_root = PathBuf::from(&request.game_root);
    if !game_root.is_absolute() {
        return Err("GTA Legacy root must be an absolute path".into());
    }
    if !game_root.is_dir() {
        return Err(format!(
            "GTA Legacy root is not a directory: {}",
            game_root.display()
        ));
    }

    let executable = game_root.join("GTA5.exe");
    if !executable.is_file() {
        return Err(format!(
            "GTA5.exe was not found in the selected Legacy root: {}",
            executable.display()
        ));
    }

    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "LOCALAPPDATA is unavailable; cannot create RageLab key cache".to_string()
        })?;
    let cache_root = local_app_data
        .join("RageLab")
        .join("cache")
        .join("gta-rpf-keys");

    prepare_gta_rpf_keys(&executable, &cache_root).map_err(core_error)
}

#[tauri::command]
fn core_prepare_gta_rpf_index(
    request: PrepareGtaRpfIndexRequest,
) -> Result<GtaRpfIndexCacheReport, String> {
    let game_root = PathBuf::from(&request.game_root);
    let keys = PathBuf::from(&request.keys);

    if !game_root.is_absolute() {
        return Err("GTA Legacy root must be an absolute path".into());
    }
    if !game_root.is_dir() {
        return Err(format!(
            "GTA Legacy root is not a directory: {}",
            game_root.display()
        ));
    }
    if !keys.is_absolute() {
        return Err("GTA RPF key store must be an absolute path".into());
    }
    if !keys.is_dir() {
        return Err(format!(
            "GTA RPF key store is not a directory: {}",
            keys.display()
        ));
    }

    let local_app_data = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .ok_or_else(|| {
            "LOCALAPPDATA is unavailable; cannot create RageLab game index cache".to_string()
        })?;
    let cache_root = local_app_data
        .join("RageLab")
        .join("cache")
        .join("gta-rpf-index");

    prepare_gta_rpf_index(&game_root, &keys, &cache_root).map_err(core_error)
}

#[tauri::command]
fn core_asset_inspect(path: String) -> Result<AssetInspectionReport, String> {
    inspect_asset(Path::new(&path)).map_err(core_error)
}

#[tauri::command]
fn core_asset_capabilities(path: String) -> Result<AssetCapabilitiesReport, String> {
    asset_capabilities(Path::new(&path)).map_err(core_error)
}

#[tauri::command]
fn core_glb_inspect(path: String) -> Result<GlbInspectionReport, String> {
    let source = require_absolute_file(&path, "GLB source")?;
    let bytes = fs::read(&source).map_err(core_error)?;
    let file_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("model.glb");
    inspect_glb_bytes(&bytes, file_name).map_err(core_error)
}

#[tauri::command]
fn core_glb_geometry(path: String) -> Result<DecodedGeometryReport, String> {
    let source = require_absolute_file(&path, "GLB source")?;
    let bytes = fs::read(&source).map_err(core_error)?;
    let file_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("model.glb");
    decode_glb_geometry_bytes(&bytes, file_name).map_err(core_error)
}

#[tauri::command]
fn core_glb_profile_init(path: String) -> Result<GtaAuthoringProfile, String> {
    let source = require_absolute_file(&path, "GLB source")?;
    let bytes = fs::read(&source).map_err(core_error)?;
    let file_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("model.glb");
    default_glb_authoring_profile(&bytes, file_name).map_err(core_error)
}

#[tauri::command]
fn core_glb_profile_validate(
    request: GlbProfileValidateRequest,
) -> Result<ProfileValidationReport, String> {
    let source = require_absolute_file(&request.source, "GLB source")?;
    let profile = require_absolute_file(&request.profile, "GLB authoring profile")?;
    let source_bytes = fs::read(&source).map_err(core_error)?;
    let profile_bytes = fs::read(&profile).map_err(core_error)?;
    let file_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("model.glb");
    validate_glb_authoring_profile_bytes(&source_bytes, file_name, &profile_bytes)
        .map_err(core_error)
}

#[tauri::command]
fn core_glb_export(request: GlbExportCommandRequest) -> Result<StudioGlbExportReport, String> {
    let source = require_absolute_file(&request.source, "GLB source")?;
    let output = PathBuf::from(&request.output);
    if !output.is_absolute() {
        return Err("Studio GLB YDR output must be an absolute path".into());
    }
    if output
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|value| !value.eq_ignore_ascii_case("ydr"))
    {
        return Err("Studio GLB YDR output must use the .ydr extension".into());
    }
    ensure_distinct_glb_paths(&source, &output)?;
    if output.exists() && !request.overwrite {
        return Err(format!(
            "Studio GLB YDR output already exists: {}",
            output.display()
        ));
    }
    let parent = output
        .parent()
        .ok_or_else(|| "Studio GLB YDR output must have a parent directory".to_string())?;
    if !parent.is_dir() {
        return Err(format!(
            "Studio GLB YDR output parent is not a directory: {}",
            parent.display()
        ));
    }

    let source_bytes = fs::read(&source).map_err(core_error)?;
    let source_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("model.glb")
        .to_owned();
    let profile_path = request
        .profile
        .as_deref()
        .map(|value| require_absolute_file(value, "GLB authoring profile"))
        .transpose()?;
    if let Some(path) = profile_path.as_ref() {
        ensure_distinct_glb_paths(path, &output)?;
    }
    let profile_bytes = profile_path
        .as_ref()
        .map(fs::read)
        .transpose()
        .map_err(core_error)?;
    let profile_snapshot = profile_bytes.clone();

    let template_path = request
        .shader_template
        .as_deref()
        .map(|value| require_absolute_file(value, "Legacy shader template"))
        .transpose()?;
    let shader_template = if let Some(path) = template_path.as_ref() {
        ensure_distinct_glb_paths(path, &output)?;
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .unwrap_or_default()
            .to_ascii_lowercase();
        let format = match extension.as_str() {
            "yft" => GlbShaderTemplateFormat::LegacyYft,
            "ydr" => GlbShaderTemplateFormat::LegacyYdr,
            _ => return Err("Legacy shader template must be a .yft or .ydr resource".into()),
        };
        Some(GlbShaderTemplateInput {
            format,
            source_name: path.display().to_string(),
            bytes: fs::read(path).map_err(core_error)?,
        })
    } else {
        None
    };
    let template_snapshot = shader_template
        .as_ref()
        .map(|template| template.bytes.clone());

    let result = export_glb_to_ydr(GlbYdrExportRequest {
        source_name,
        source_bytes: source_bytes.clone(),
        profile_bytes,
        shader_template,
        shader_index: request.shader_index.unwrap_or(0),
    })
    .map_err(core_error)?;

    verify_glb_input_unchanged(&source, &source_bytes)?;
    if let (Some(path), Some(expected)) = (profile_path.as_ref(), profile_snapshot.as_deref()) {
        verify_glb_input_unchanged(path, expected)?;
    }
    if let (Some(path), Some(expected)) = (template_path.as_ref(), template_snapshot.as_deref()) {
        verify_glb_input_unchanged(path, expected)?;
    }

    let mut options = fs::OpenOptions::new();
    options.write(true);
    if request.overwrite {
        options.create(true).truncate(true);
    } else {
        options.create_new(true);
    }
    let mut file = options.open(&output).map_err(core_error)?;
    file.write_all(&result.output_bytes).map_err(core_error)?;
    file.sync_all().map_err(core_error)?;

    Ok(StudioGlbExportReport {
        source: source.display().to_string(),
        output: output.display().to_string(),
        bytes_written: result.output_bytes.len(),
        engine: result.report,
    })
}

fn require_absolute_file(path: &str, label: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(path);
    if !path.is_absolute() {
        return Err(format!("{label} must be an absolute path"));
    }
    if !path.is_file() {
        return Err(format!("{label} is not a file: {}", path.display()));
    }
    Ok(path)
}

fn ensure_distinct_glb_paths(input: &Path, output: &Path) -> Result<(), String> {
    let input = input.canonicalize().map_err(core_error)?;
    let output_abs = if output.exists() {
        output.canonicalize().map_err(core_error)?
    } else {
        let parent = output
            .parent()
            .filter(|value| !value.as_os_str().is_empty())
            .unwrap_or(Path::new("."))
            .canonicalize()
            .map_err(core_error)?;
        let name = output
            .file_name()
            .ok_or_else(|| "GLB YDR output must name a file".to_string())?;
        parent.join(name)
    };
    if input == output_abs {
        return Err("GLB YDR output must not overwrite any input resource".into());
    }
    Ok(())
}

fn verify_glb_input_unchanged(path: &Path, expected: &[u8]) -> Result<(), String> {
    if fs::read(path).map_err(core_error)? != expected {
        return Err(format!(
            "GLB export input changed during processing: {}",
            path.display()
        ));
    }
    Ok(())
}

#[tauri::command]
fn core_ytd_export_png(
    request: YtdTexturePngExportRequest,
) -> Result<YtdTexturePngExportReport, String> {
    let source = PathBuf::from(&request.path);
    let output = PathBuf::from(&request.output);
    if !source.is_absolute() || !source.is_file() {
        return Err(format!(
            "Studio YTD PNG source must be an absolute file path: {}",
            source.display()
        ));
    }
    if !output.is_absolute() {
        return Err("Studio YTD PNG output must be an absolute path".into());
    }
    if source == output {
        return Err("Studio YTD PNG output must differ from the source path".into());
    }
    if output.exists() {
        return Err(format!(
            "Studio YTD PNG output already exists: {}",
            output.display()
        ));
    }
    let parent = output
        .parent()
        .ok_or_else(|| "Studio YTD PNG output must have a parent directory".to_string())?;
    if !parent.is_dir() {
        return Err(format!(
            "Studio YTD PNG output parent is not a directory: {}",
            parent.display()
        ));
    }

    let source_bytes = fs::read(&source).map_err(core_error)?;
    let png = export_ytd_texture_png(&source_bytes, request.texture_index).map_err(core_error)?;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&output)
        .map_err(core_error)?;
    file.write_all(&png).map_err(core_error)?;
    file.sync_all().map_err(core_error)?;

    let source_unchanged = fs::read(&source).map_err(core_error)? == source_bytes;
    if !source_unchanged {
        return Err("YTD source changed during read-only PNG export".into());
    }

    Ok(YtdTexturePngExportReport {
        output: output.display().to_string(),
        bytes_written: png.len(),
        source_unchanged,
    })
}

#[tauri::command]
fn core_asset_preview(request: PreviewRequest) -> Result<AssetPreviewReport, String> {
    preview_asset(Path::new(&request.path), request.options()).map_err(core_error)
}

#[tauri::command]
fn core_workspace_scene(request: WorkspaceSceneRequest) -> Result<SceneManifestReport, String> {
    let scene = validate_workspace_scene_context(
        &request.workspace,
        &request.ymap,
        &request.fallback_roots,
        &request.rpf_mounts,
        request.game_index.as_ref(),
        request.max_nodes,
    )?;
    workspace_scene_report_with_game_index(
        &scene.workspace,
        &scene.ymap,
        &scene.fallback_roots,
        &scene.rpf_mounts,
        scene.game_index.as_ref(),
        scene.options,
    )
    .map_err(core_error)
}

#[tauri::command]
fn core_workspace_scene_asset_preview(
    request: WorkspaceSceneAssetPreviewRequest,
) -> Result<AssetPreviewReport, String> {
    let scene = validate_workspace_scene_context(
        &request.workspace,
        &request.ymap,
        &request.fallback_roots,
        &request.rpf_mounts,
        request.game_index.as_ref(),
        request.max_nodes,
    )?;
    workspace_scene_asset_preview_with_game_index(
        &scene.workspace,
        &scene.ymap,
        SceneAssetPreviewSources {
            fallback_roots: &scene.fallback_roots,
            rpf_mounts: &scene.rpf_mounts,
            game_index: scene.game_index.as_ref(),
        },
        scene.options,
        request.asset_ref,
        PreviewOptions::default(),
    )
    .map_err(core_error)
}

fn validate_workspace_scene_context(
    workspace: &str,
    ymap: &str,
    fallback_roots: &[String],
    rpf_mounts: &[SceneRpfMountRequest],
    game_index: Option<&SceneGameIndexRequest>,
    max_nodes: Option<usize>,
) -> Result<ValidatedWorkspaceScene, String> {
    let workspace = PathBuf::from(workspace);
    let ymap = PathBuf::from(ymap);
    let fallback_roots = fallback_roots.iter().map(PathBuf::from).collect::<Vec<_>>();

    if !workspace.is_absolute() {
        return Err("Studio scene workspace must be an absolute path".into());
    }
    if !ymap.is_absolute() {
        return Err("Studio scene YMAP must be an absolute path".into());
    }
    for fallback_root in &fallback_roots {
        if !fallback_root.is_absolute() {
            return Err(format!(
                "Studio scene fallback root must be an absolute path: {}",
                fallback_root.display()
            ));
        }
        if !fallback_root.is_dir() {
            return Err(format!(
                "Studio scene fallback root is not a directory: {}",
                fallback_root.display()
            ));
        }
    }

    let mut validated_rpf_mounts = Vec::with_capacity(rpf_mounts.len());
    for mount in rpf_mounts {
        let archive = PathBuf::from(&mount.archive);
        let keys = PathBuf::from(&mount.keys);
        if !archive.is_absolute() {
            return Err(format!(
                "Studio scene RPF archive must be an absolute path: {}",
                archive.display()
            ));
        }
        if !archive.is_file() {
            return Err(format!(
                "Studio scene RPF archive is not a file: {}",
                archive.display()
            ));
        }
        if !archive
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("rpf"))
        {
            return Err(format!(
                "Studio scene RPF archive must use the .rpf extension: {}",
                archive.display()
            ));
        }
        if !keys.is_absolute() {
            return Err(format!(
                "Studio scene RPF keys must be an absolute directory: {}",
                keys.display()
            ));
        }
        if !keys.is_dir() {
            return Err(format!(
                "Studio scene RPF keys directory does not exist: {}",
                keys.display()
            ));
        }
        if mount.nested.iter().any(|entry| entry.trim().is_empty()) {
            return Err("Studio scene nested RPF paths must not be empty".into());
        }

        validated_rpf_mounts.push(SceneRpfMount::new(archive, mount.nested.clone(), keys));
    }

    let validated_game_index = if let Some(source) = game_index {
        let game_root = PathBuf::from(&source.game_root);
        let index = PathBuf::from(&source.index);
        let keys = PathBuf::from(&source.keys);

        if !game_root.is_absolute() || !game_root.is_dir() {
            return Err(format!(
                "Studio scene game root is invalid: {}",
                game_root.display()
            ));
        }
        if !index.is_absolute() || !index.is_file() {
            return Err(format!(
                "Studio scene game index is invalid: {}",
                index.display()
            ));
        }
        if !keys.is_absolute() || !keys.is_dir() {
            return Err(format!(
                "Studio scene game key store is invalid: {}",
                keys.display()
            ));
        }

        Some(SceneGameIndexSource::new(game_root, index, keys))
    } else {
        None
    };

    if !workspace.is_dir() {
        return Err(format!(
            "Studio scene workspace is not a directory: {}",
            workspace.display()
        ));
    }
    if !ymap.is_file() {
        return Err(format!(
            "Studio scene YMAP is not a file: {}",
            ymap.display()
        ));
    }
    if !ymap
        .extension()
        .and_then(|value| value.to_str())
        .is_some_and(|extension| extension.eq_ignore_ascii_case("ymap"))
    {
        return Err("Studio scene input must use the .ymap extension".into());
    }

    let max_nodes = max_nodes.unwrap_or_else(|| SceneAssemblyOptions::default().max_nodes);
    if max_nodes == 0 {
        return Err("Studio scene maxNodes must be greater than zero".into());
    }
    if max_nodes > MAX_SCENE_NODE_LIMIT {
        return Err(format!(
            "Studio scene maxNodes exceeds hard limit {MAX_SCENE_NODE_LIMIT}"
        ));
    }

    let canonical_workspace = fs::canonicalize(&workspace)
        .map_err(|error| format!("Unable to canonicalize Studio workspace: {error}"))?;
    let canonical_ymap = fs::canonicalize(&ymap)
        .map_err(|error| format!("Unable to canonicalize Studio YMAP: {error}"))?;
    if !canonical_ymap.starts_with(&canonical_workspace) {
        return Err("Studio scene YMAP must be contained by the selected workspace".into());
    }

    Ok(ValidatedWorkspaceScene {
        workspace,
        ymap,
        fallback_roots,
        rpf_mounts: validated_rpf_mounts,
        game_index: validated_game_index,
        options: SceneAssemblyOptions::new(max_nodes),
    })
}

#[tauri::command]
fn core_workspace_export_preflight(
    request: WorkspaceExportPreflightRequest,
) -> Result<WorkspaceExportPreflightReport, String> {
    let (workspace, maps) = validate_workspace_export_selection(&request.workspace, &request.maps)?;
    workspace_export_preflight_report(&workspace, &maps, CatalogRefs::default()).map_err(core_error)
}

#[tauri::command]
fn core_workspace_export(request: WorkspaceExportRequest) -> Result<WorkspaceExportReport, String> {
    let (workspace, maps) = validate_workspace_export_selection(&request.workspace, &request.maps)?;
    let output = validate_workspace_export_output(&workspace, &request.output)?;
    let resource_name = request.resource_name.trim();
    if resource_name.is_empty() {
        return Err("Studio export resourceName must not be empty".into());
    }

    workspace_export_report(
        &workspace,
        &maps,
        &output,
        resource_name,
        SharedExportOptions {
            allow_unresolved: request.allow_unresolved,
            overwrite: false,
        },
        CatalogRefs::default(),
    )
    .map_err(core_error)
}

fn validate_workspace_export_selection(
    workspace: &str,
    maps: &[String],
) -> Result<(PathBuf, Vec<PathBuf>), String> {
    let workspace = PathBuf::from(workspace);
    if !workspace.is_absolute() {
        return Err("Studio export workspace must be an absolute path".into());
    }
    if !workspace.is_dir() {
        return Err(format!(
            "Studio export workspace is not a directory: {}",
            workspace.display()
        ));
    }
    if maps.is_empty() {
        return Err("Studio export requires at least one YMAP".into());
    }

    let mut map_paths = Vec::with_capacity(maps.len());
    for map in maps {
        if map.trim().is_empty() {
            return Err("Studio export YMAP path must not be empty".into());
        }
        map_paths.push(PathBuf::from(map));
    }

    Ok((workspace, map_paths))
}

fn validate_workspace_export_output(workspace: &Path, output: &str) -> Result<PathBuf, String> {
    let output = PathBuf::from(output);
    if !output.is_absolute() {
        return Err("Studio export output must be an absolute path".into());
    }
    if output.exists() {
        return Err(format!(
            "Studio export output must not already exist: {}",
            output.display()
        ));
    }

    let file_name = output
        .file_name()
        .ok_or_else(|| "Studio export output must name a resource directory".to_string())?;
    let parent = output
        .parent()
        .ok_or_else(|| "Studio export output must have a parent directory".to_string())?;
    if !parent.is_dir() {
        return Err(format!(
            "Studio export output parent is not a directory: {}",
            parent.display()
        ));
    }

    let canonical_workspace = fs::canonicalize(workspace)
        .map_err(|error| format!("Unable to canonicalize Studio workspace: {error}"))?;
    let canonical_parent = fs::canonicalize(parent)
        .map_err(|error| format!("Unable to canonicalize Studio export parent: {error}"))?;
    let canonical_output = canonical_parent.join(file_name);
    if canonical_output.starts_with(&canonical_workspace) {
        return Err("Studio export output must be outside the source workspace".into());
    }

    Ok(output)
}

#[tauri::command]
fn core_operation_plan(document: OperationDocument) -> Result<OperationPlan, String> {
    ensure_studio_operation_paths(&document)?;
    plan_operation_document(&document, Path::new(".")).map_err(core_error)
}

#[tauri::command]
fn core_operation_apply(document: OperationDocument) -> Result<OperationApplyResult, String> {
    ensure_studio_operation_paths(&document)?;
    apply_operation_document(&document, Path::new(".")).map_err(core_error)
}

fn ensure_studio_operation_paths(document: &OperationDocument) -> Result<(), String> {
    if !document.source.is_absolute() {
        return Err("Studio operation source must be an absolute path".into());
    }
    if !document.output.is_absolute() {
        return Err("Studio operation output must be an absolute path".into());
    }
    if document.source == document.output {
        return Err("Studio operation output must differ from the source path".into());
    }

    for operation in &document.operations {
        if let Some(replacement) = operation
            .parameters
            .get("replacement")
            .and_then(|value| value.as_str())
        {
            if !Path::new(replacement).is_absolute() {
                return Err(format!(
                    "Studio operation replacement payload must be an absolute path: {replacement}"
                ));
            }
        }
    }

    Ok(())
}

fn core_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    diagnostics::init(CORE_REVISION);

    let result = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(NativeViewportState::default())
        .manage(AuthoringState::default())
        .setup(|app| {
            native_viewport::maybe_start_viewport_smoke(app.handle().clone(), CORE_REVISION);
            #[cfg(debug_assertions)]
            {
                native_viewport::maybe_start_debug_open(app.handle().clone());
                native_viewport::maybe_start_debug_world_open(app.handle().clone());
                native_viewport::maybe_start_debug_world_stream_smoke(
                    app.handle().clone(),
                    CORE_REVISION,
                );
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            studio_info,
            core_discover_gta_legacy,
            core_discover_fivem_legacy,
            core_prepare_gta_rpf_keys,
            core_prepare_gta_rpf_index,
            core_asset_inspect,
            core_asset_capabilities,
            core_glb_inspect,
            core_glb_geometry,
            core_glb_profile_init,
            core_glb_profile_validate,
            core_glb_export,
            core_ytd_export_png,
            core_asset_preview,
            core_workspace_scene,
            core_workspace_scene_asset_preview,
            authoring_open,
            authoring_snapshot,
            authoring_apply,
            authoring_undo,
            authoring_redo,
            authoring_revert,
            authoring_save_as,
            authoring_preview,
            authoring_preview_world_overlay,
            authoring_close,
            native_viewport_create,
            native_viewport_create_dedicated,
            native_viewport_open_dedicated_scene,
            native_viewport_open_dedicated_world,
            native_viewport_load_scene,
            native_viewport_start_world_stream,
            native_viewport_world_stream_at,
            native_viewport_world_search,
            native_viewport_world_set_workspace_overlay,
            native_viewport_world_clear_overlays,
            native_viewport_stop_world_stream,
            native_viewport_set_rect,
            native_viewport_input,
            native_viewport_pick,
            native_viewport_select,
            native_viewport_focus_node,
            native_viewport_focus_window,
            native_viewport_set_node_visible,
            native_viewport_isolate_node,
            native_viewport_show_all_nodes,
            native_viewport_set_layer_visibility,
            native_viewport_set_projection,
            native_viewport_set_overlays,
            native_viewport_set_gizmo,
            native_viewport_set_visible,
            native_viewport_stats,
            native_viewport_shutdown,
            debug_three_viewport_benchmark_spec,
            debug_three_viewport_benchmark_complete,
            core_workspace_export_preflight,
            core_workspace_export,
            core_operation_plan,
            core_operation_apply,
            diagnostics_summary,
            diagnostics_record_frontend_error
        ])
        .run(tauri::generate_context!());

    if result.is_ok() {
        diagnostics::mark_graceful_exit();
    }

    result.expect("error while running RageLab Studio");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_request_uses_engine_defaults_and_only_overrides_explicit_fields() {
        let defaults = PreviewOptions::default();
        let request = PreviewRequest {
            path: "asset.ydr".into(),
            max_vertices: Some(42),
            ..PreviewRequest::default()
        };

        let options = request.options();
        assert_eq!(options.max_vertices, 42);
        assert_eq!(options.max_indices, defaults.max_indices);
        assert_eq!(options.max_primitives, defaults.max_primitives);
        assert_eq!(options.max_children, defaults.max_children);
        assert_eq!(options.drawable_index, None);
    }

    #[test]
    fn studio_info_exposes_core_build_metadata_and_api_compatibility() {
        let info = studio_info();
        assert_eq!(info.core_revision, CORE_REVISION);
        assert!(!info.core_revision.is_empty());
        assert_eq!(info.core_dirty, CORE_DIRTY);
        assert_eq!(info.core_api_compatibility, CORE_API_COMPATIBILITY_VERSION);
        assert!(info.core_api_compatibility > 0);
    }

    #[test]
    fn export_glb_typescript_bindings() {
        use ts_rs::{Config, TS};

        let output = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../src/lib/generated/glb");
        let config = Config::new().with_out_dir(output);

        StudioInfo::export_all(&config).unwrap();
        GlbProfileValidateRequest::export_all(&config).unwrap();
        GlbExportCommandRequest::export_all(&config).unwrap();
        StudioGlbExportReport::export_all(&config).unwrap();
        GlbInspectionReport::export_all(&config).unwrap();
        DecodedGeometryReport::export_all(&config).unwrap();
        GtaAuthoringProfile::export_all(&config).unwrap();
        ProfileValidationReport::export_all(&config).unwrap();
        GlbYdrExportReport::export_all(&config).unwrap();
    }

    #[test]
    fn glb_tauri_adapter_uses_engine_pipeline_without_overwriting_inputs_or_outputs() {
        let base = std::env::temp_dir().join(format!("ragelab-studio-glb-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&base).unwrap();
        let source = base.join("triangle.glb");
        let profile_path = base.join("triangle.profile.json");
        let output = base.join("triangle.ydr");
        let source_bytes = triangle_glb_bytes();
        fs::write(&source, &source_bytes).unwrap();

        let inspection = core_glb_inspect(source.display().to_string()).unwrap();
        assert_eq!(inspection.source.file_name, "triangle.glb");
        let geometry = core_glb_geometry(source.display().to_string()).unwrap();
        assert_eq!(geometry.primitives.len(), 1);
        assert_eq!(geometry.primitives[0].vertex_count, 3);

        let profile = core_glb_profile_init(source.display().to_string()).unwrap();
        fs::write(&profile_path, serde_json::to_vec_pretty(&profile).unwrap()).unwrap();
        let validation = core_glb_profile_validate(GlbProfileValidateRequest {
            source: source.display().to_string(),
            profile: profile_path.display().to_string(),
        })
        .unwrap();
        assert!(validation.valid);

        let result = core_glb_export(GlbExportCommandRequest {
            source: source.display().to_string(),
            output: output.display().to_string(),
            profile: Some(profile_path.display().to_string()),
            shader_template: None,
            shader_index: None,
            overwrite: false,
        })
        .unwrap();
        assert!(result.engine.semantic_reopen);
        assert!(!result.engine.runtime_renderable);
        assert!(output.is_file());
        assert_eq!(
            result.bytes_written,
            fs::metadata(&output).unwrap().len() as usize
        );
        assert_eq!(fs::read(&source).unwrap(), source_bytes);

        let refused_overwrite = core_glb_export(GlbExportCommandRequest {
            source: source.display().to_string(),
            output: output.display().to_string(),
            profile: Some(profile_path.display().to_string()),
            shader_template: None,
            shader_index: None,
            overwrite: false,
        })
        .unwrap_err();
        assert!(refused_overwrite.contains("already exists"));
        assert_eq!(fs::read(&source).unwrap(), source_bytes);

        fs::remove_dir_all(base).unwrap();
    }

    fn triangle_glb_bytes() -> Vec<u8> {
        let mut binary = Vec::new();
        for vertex in [[0.0_f32, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]] {
            for component in vertex {
                binary.extend_from_slice(&component.to_le_bytes());
            }
        }
        let indices_offset = binary.len();
        for index in [0_u16, 1, 2] {
            binary.extend_from_slice(&index.to_le_bytes());
        }
        while binary.len() % 4 != 0 {
            binary.push(0);
        }

        let document = serde_json::json!({
            "asset": {"version": "2.0", "generator": "RageLab Studio Tauri integration test"},
            "scene": 0,
            "scenes": [{"nodes": [0]}],
            "nodes": [{"name": "Triangle", "mesh": 0}],
            "meshes": [{
                "name": "TriangleMesh",
                "primitives": [{
                    "attributes": {"POSITION": 0},
                    "indices": 1
                }]
            }],
            "accessors": [
                {
                    "bufferView": 0,
                    "componentType": 5126,
                    "count": 3,
                    "type": "VEC3",
                    "min": [0.0, 0.0, 0.0],
                    "max": [1.0, 1.0, 0.0]
                },
                {
                    "bufferView": 1,
                    "componentType": 5123,
                    "count": 3,
                    "type": "SCALAR"
                }
            ],
            "bufferViews": [
                {"buffer": 0, "byteOffset": 0, "byteLength": 36},
                {"buffer": 0, "byteOffset": indices_offset, "byteLength": 6}
            ],
            "buffers": [{"byteLength": binary.len()}]
        });
        let mut json_chunk = serde_json::to_vec(&document).unwrap();
        while !json_chunk.len().is_multiple_of(4) {
            json_chunk.push(b' ');
        }
        let total_length = 12 + 8 + json_chunk.len() + 8 + binary.len();
        let mut bytes = Vec::with_capacity(total_length);
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&(total_length as u32).to_le_bytes());
        bytes.extend_from_slice(&(json_chunk.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x4E4F534A_u32.to_le_bytes());
        bytes.extend_from_slice(&json_chunk);
        bytes.extend_from_slice(&(binary.len() as u32).to_le_bytes());
        bytes.extend_from_slice(&0x004E4942_u32.to_le_bytes());
        bytes.extend_from_slice(&binary);
        bytes
    }

    #[test]
    fn workspace_scene_request_enforces_paths_mounts_and_limits() {
        let base =
            std::env::temp_dir().join(format!("ragelab-studio-scene-{}", std::process::id()));
        let workspace = base.join("workspace");
        let inside = workspace.join("map.ymap");
        let outside = base.join("outside.ymap");
        let archive = base.join("game.rpf");
        let keys = base.join("keys");

        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&workspace).unwrap();
        fs::create_dir_all(&keys).unwrap();
        fs::write(&inside, b"fixture").unwrap();
        fs::write(&outside, b"fixture").unwrap();
        fs::write(&archive, b"fixture").unwrap();

        let mounts = vec![SceneRpfMountRequest {
            archive: archive.display().to_string(),
            nested: vec!["nested/content.rpf".into()],
            keys: keys.display().to_string(),
        }];
        let valid = validate_workspace_scene_context(
            &workspace.display().to_string(),
            &inside.display().to_string(),
            &[],
            &mounts,
            None,
            Some(42),
        )
        .unwrap();
        assert!(valid.fallback_roots.is_empty());
        assert_eq!(valid.rpf_mounts.len(), 1);
        assert!(valid.game_index.is_none());
        assert_eq!(valid.options.max_nodes, 42);

        assert!(
            validate_workspace_scene_context("relative", "map.ymap", &[], &[], None, None).is_err()
        );
        assert!(validate_workspace_scene_context(
            &workspace.display().to_string(),
            &outside.display().to_string(),
            &[],
            &[],
            None,
            None,
        )
        .is_err());
        assert!(validate_workspace_scene_context(
            &workspace.display().to_string(),
            &inside.display().to_string(),
            &[],
            &[],
            None,
            Some(0),
        )
        .is_err());
        assert!(validate_workspace_scene_context(
            &workspace.display().to_string(),
            &inside.display().to_string(),
            &[],
            &[],
            None,
            Some(MAX_SCENE_NODE_LIMIT + 1),
        )
        .is_err());

        let invalid_mount = vec![SceneRpfMountRequest {
            archive: base.join("missing.rpf").display().to_string(),
            nested: Vec::new(),
            keys: keys.display().to_string(),
        }];
        assert!(validate_workspace_scene_context(
            &workspace.display().to_string(),
            &inside.display().to_string(),
            &[],
            &invalid_mount,
            None,
            None,
        )
        .is_err());

        let index = base.join("game-index.bin");
        fs::write(&index, b"fixture").unwrap();
        let game_index = SceneGameIndexRequest {
            game_root: base.display().to_string(),
            index: index.display().to_string(),
            keys: keys.display().to_string(),
        };
        let valid = validate_workspace_scene_context(
            &workspace.display().to_string(),
            &inside.display().to_string(),
            &[],
            &[],
            Some(&game_index),
            None,
        )
        .unwrap();
        assert!(valid.game_index.is_some());

        let invalid_index = SceneGameIndexRequest {
            game_root: base.display().to_string(),
            index: base.join("missing.bin").display().to_string(),
            keys: keys.display().to_string(),
        };
        assert!(validate_workspace_scene_context(
            &workspace.display().to_string(),
            &inside.display().to_string(),
            &[],
            &[],
            Some(&invalid_index),
            None,
        )
        .is_err());

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn workspace_export_request_enforces_selection_and_create_new_output() {
        let base =
            std::env::temp_dir().join(format!("ragelab-studio-export-{}", std::process::id()));
        let workspace = base.join("workspace");
        let map = workspace.join("map.ymap");
        let outside_output = base.join("resource-output");
        let inside_output = workspace.join("resource-output");

        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&workspace).unwrap();
        fs::write(&map, b"fixture").unwrap();

        let (_, maps) = validate_workspace_export_selection(
            &workspace.display().to_string(),
            &[map.display().to_string()],
        )
        .unwrap();
        assert_eq!(maps, vec![map.clone()]);

        assert!(validate_workspace_export_selection("relative", &[]).is_err());
        assert!(
            validate_workspace_export_selection(&workspace.display().to_string(), &[]).is_err()
        );

        let valid_output =
            validate_workspace_export_output(&workspace, &outside_output.display().to_string())
                .unwrap();
        assert_eq!(valid_output, outside_output);

        assert!(
            validate_workspace_export_output(&workspace, &inside_output.display().to_string())
                .is_err()
        );
        assert!(validate_workspace_export_output(&workspace, "relative-output").is_err());

        fs::create_dir_all(&outside_output).unwrap();
        assert!(validate_workspace_export_output(
            &workspace,
            &outside_output.display().to_string()
        )
        .is_err());

        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn operation_paths_must_be_absolute_and_non_destructive() {
        let relative = OperationDocument {
            schema: "ragelab.operation".into(),
            schema_version: 1,
            source: "source.ydr".into(),
            output: "output.ydr".into(),
            operations: vec![],
        };
        assert!(ensure_studio_operation_paths(&relative).is_err());

        let source = std::env::current_dir().unwrap().join("source.ydr");
        let same = OperationDocument {
            schema: "ragelab.operation".into(),
            schema_version: 1,
            source: source.clone(),
            output: source,
            operations: vec![],
        };
        assert!(ensure_studio_operation_paths(&same).is_err());

        let valid = OperationDocument {
            schema: "ragelab.operation".into(),
            schema_version: 1,
            source: std::env::current_dir().unwrap().join("source.ydr"),
            output: std::env::current_dir().unwrap().join("output.ydr"),
            operations: vec![],
        };
        assert!(ensure_studio_operation_paths(&valid).is_ok());
    }
}
