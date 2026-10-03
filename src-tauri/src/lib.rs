use std::{
    fs,
    path::{Path, PathBuf},
};

use ragelab_engine::{
    apply_operation_document, asset_capabilities, discover_fivem_legacy, discover_gta_v_legacy,
    inspect_asset, plan_operation_document, preview_asset, workspace_export_preflight_report,
    workspace_export_report, workspace_scene_report, AssetCapabilitiesReport,
    AssetInspectionReport, AssetPreviewReport, CatalogRefs, FiveMDiscoveryReport,
    GtaVDiscoveryReport, OperationApplyResult, OperationDocument, OperationPlan, PreviewOptions,
    SceneAssemblyOptions, SceneManifestReport, SharedExportOptions, WorkspaceExportPreflightReport,
    WorkspaceExportReport, MAX_SCENE_NODE_LIMIT,
};
use serde::{Deserialize, Serialize};

const RAGELAB_CORE_REVISION: &str = "c8264587a4daf864b8d3f1b88727f53cab942573";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioInfo {
    product: &'static str,
    version: &'static str,
    core_revision: &'static str,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WorkspaceSceneRequest {
    workspace: String,
    ymap: String,
    max_nodes: Option<usize>,
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
        let mut options = PreviewOptions::default();
        options.drawable_index = self.drawable_index;

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
        core_revision: RAGELAB_CORE_REVISION,
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
fn core_asset_inspect(path: String) -> Result<AssetInspectionReport, String> {
    inspect_asset(Path::new(&path)).map_err(core_error)
}

#[tauri::command]
fn core_asset_capabilities(path: String) -> Result<AssetCapabilitiesReport, String> {
    asset_capabilities(Path::new(&path)).map_err(core_error)
}

#[tauri::command]
fn core_asset_preview(request: PreviewRequest) -> Result<AssetPreviewReport, String> {
    preview_asset(Path::new(&request.path), request.options()).map_err(core_error)
}

#[tauri::command]
fn core_workspace_scene(request: WorkspaceSceneRequest) -> Result<SceneManifestReport, String> {
    let (workspace, ymap, options) = validate_workspace_scene_request(&request)?;
    workspace_scene_report(&workspace, &ymap, options).map_err(core_error)
}

fn validate_workspace_scene_request(
    request: &WorkspaceSceneRequest,
) -> Result<(PathBuf, PathBuf, SceneAssemblyOptions), String> {
    let workspace = PathBuf::from(&request.workspace);
    let ymap = PathBuf::from(&request.ymap);

    if !workspace.is_absolute() {
        return Err("Studio scene workspace must be an absolute path".into());
    }
    if !ymap.is_absolute() {
        return Err("Studio scene YMAP must be an absolute path".into());
    }
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

    let max_nodes = request
        .max_nodes
        .unwrap_or_else(|| SceneAssemblyOptions::default().max_nodes);
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

    Ok((workspace, ymap, SceneAssemblyOptions::new(max_nodes)))
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
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            studio_info,
            core_discover_gta_legacy,
            core_discover_fivem_legacy,
            core_asset_inspect,
            core_asset_capabilities,
            core_asset_preview,
            core_workspace_scene,
            core_workspace_export_preflight,
            core_workspace_export,
            core_operation_plan,
            core_operation_apply
        ])
        .run(tauri::generate_context!())
        .expect("error while running RageLab Studio");
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
    fn studio_info_exposes_pinned_core_revision() {
        let info = studio_info();
        assert_eq!(info.core_revision, RAGELAB_CORE_REVISION);
    }

    #[test]
    fn workspace_scene_request_enforces_absolute_contained_ymap_and_limits() {
        let base =
            std::env::temp_dir().join(format!("ragelab-studio-scene-{}", std::process::id()));
        let workspace = base.join("workspace");
        let inside = workspace.join("map.ymap");
        let outside = base.join("outside.ymap");

        let _ = fs::remove_dir_all(&base);
        fs::create_dir_all(&workspace).unwrap();
        fs::write(&inside, b"fixture").unwrap();
        fs::write(&outside, b"fixture").unwrap();

        let valid = WorkspaceSceneRequest {
            workspace: workspace.display().to_string(),
            ymap: inside.display().to_string(),
            max_nodes: Some(42),
        };
        let (_, _, options) = validate_workspace_scene_request(&valid).unwrap();
        assert_eq!(options.max_nodes, 42);

        let relative = WorkspaceSceneRequest {
            workspace: "relative".into(),
            ymap: "map.ymap".into(),
            max_nodes: None,
        };
        assert!(validate_workspace_scene_request(&relative).is_err());

        let outside_request = WorkspaceSceneRequest {
            workspace: workspace.display().to_string(),
            ymap: outside.display().to_string(),
            max_nodes: None,
        };
        assert!(validate_workspace_scene_request(&outside_request).is_err());

        let zero = WorkspaceSceneRequest {
            workspace: workspace.display().to_string(),
            ymap: inside.display().to_string(),
            max_nodes: Some(0),
        };
        assert!(validate_workspace_scene_request(&zero).is_err());

        let too_large = WorkspaceSceneRequest {
            workspace: workspace.display().to_string(),
            ymap: inside.display().to_string(),
            max_nodes: Some(MAX_SCENE_NODE_LIMIT + 1),
        };
        assert!(validate_workspace_scene_request(&too_large).is_err());

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
