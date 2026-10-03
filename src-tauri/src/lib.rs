use std::path::Path;

use ragelab_engine::{
    asset_capabilities, discover_fivem_legacy, discover_gta_v_legacy, inspect_asset, preview_asset,
    AssetCapabilitiesReport, AssetInspectionReport, AssetPreviewReport, FiveMDiscoveryReport,
    GtaVDiscoveryReport, PreviewOptions,
};
use serde::{Deserialize, Serialize};

const RAGELAB_CORE_REVISION: &str = "15db923f11ab87737984c93f97e486b766bd4959";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioInfo {
    product: &'static str,
    version: &'static str,
    core_revision: &'static str,
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
            core_asset_preview
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
}
