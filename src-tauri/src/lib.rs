use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StudioInfo {
    product: &'static str,
    version: &'static str,
}

#[tauri::command]
fn studio_info() -> StudioInfo {
    StudioInfo {
        product: "RageLab Studio",
        version: env!("CARGO_PKG_VERSION"),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![studio_info])
        .run(tauri::generate_context!())
        .expect("error while running RageLab Studio");
}
