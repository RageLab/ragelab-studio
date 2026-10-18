use std::{
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc::{self, Receiver, Sender},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use ragelab_engine::{
    workspace_scene_render_package_with_game_index, GtaRpfBrowserSearchReport, GtaRpfWorldPoint,
    RenderPackage, RenderPackageOptions, SceneAssetPreviewSources, SceneGameIndexSource,
    WorldStreamConfig, WorldStreamReport, WorldStreamUpdate, WorldStreamView,
    WorldStreamingRuntime,
};
use ragelab_render::{
    CameraSnapshot, GpuCacheBudget, PickResult, Projection, SurfaceRenderer, TransformGizmoMode,
    ViewportOptions, ViewportStats,
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition, PhysicalSize, Window, WindowEvent};

use super::{validate_workspace_scene_context, SceneGameIndexRequest, WorkspaceSceneRequest};
use crate::diagnostics;

const NATIVE_VIEWPORT_LABEL: &str = "native-viewport";
static NATIVE_VIEWPORT_GENERATION: AtomicU64 = AtomicU64::new(1);
const RENDER_THREAD_READY_TIMEOUT: Duration = Duration::from_secs(30);
const WORLD_CAMERA_FOCUS_RADIUS: f32 = 120.0;
const RENDER_COMMAND_TIMEOUT: Duration = Duration::from_secs(180);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum NativeViewportWindowMode {
    Overlay,
    Dedicated,
    DedicatedLoading,
    Standalone,
}

#[derive(Default)]
pub(crate) struct NativeViewportState {
    runtime: Mutex<Option<NativeViewportRuntime>>,
    smoke_active: AtomicBool,
}

struct NativeViewportRuntime {
    window: Window,
    sender: Sender<RenderCommand>,
    thread: Option<JoinHandle<()>>,
    input_stop: Option<Arc<AtomicBool>>,
    input_thread: Option<JoinHandle<()>>,
}

impl NativeViewportRuntime {
    fn shutdown(mut self) {
        if let Some(stop) = self.input_stop.take() {
            stop.store(true, Ordering::Release);
        }
        if let Some(thread) = self.input_thread.take() {
            let _ = thread.join();
        }
        let _ = self.sender.send(RenderCommand::Shutdown);
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        let _ = self.window.close();
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportCreateRequest {
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportRectRequest {
    x: i32,
    y: i32,
    width: u32,
    height: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportInputRequest {
    kind: String,
    #[serde(default)]
    delta_x: f32,
    #[serde(default)]
    delta_y: f32,
    #[serde(default)]
    delta: f32,
    #[serde(default)]
    forward: f32,
    #[serde(default)]
    right: f32,
    #[serde(default)]
    up: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportPickRequest {
    x: f32,
    y: f32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportSelectRequest {
    node_index: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportProjectionRequest {
    projection: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportOverlaysRequest {
    grid: bool,
    wireframe: bool,
    bounds: bool,
    collision: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportGizmoRequest {
    mode: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportVisibleRequest {
    visible: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeWorldStreamStartRequest {
    game_index: SceneGameIndexRequest,
    position: [f32; 3],
    load_radius: Option<f32>,
    retain_radius: Option<f32>,
    max_active_maps: Option<usize>,
    max_chunk_loads_per_update: Option<usize>,
    max_cpu_chunks: Option<usize>,
    max_cpu_bytes: Option<u64>,
    max_gpu_assets: Option<u32>,
    max_gpu_textures: Option<u32>,
    max_gpu_asset_bytes: Option<u64>,
    max_gpu_texture_bytes: Option<u64>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeWorldStreamAtRequest {
    position: [f32; 3],
    #[serde(default)]
    fit_camera: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeWorldSearchRequest {
    query: String,
    limit: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportNodeRequest {
    node_index: u32,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportNodeVisibilityRequest {
    node_index: u32,
    visible: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportLayerVisibilityRequest {
    base_game: bool,
    local_overlays: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportReport {
    stats: ViewportStats,
    camera: Option<CameraSnapshot>,
    streaming: Option<WorldStreamReport>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportPickReport {
    pick: Option<PickResult>,
    viewport: NativeViewportReport,
}

struct BackgroundWorldUpdate {
    position: [f32; 3],
    elapsed_ms: f64,
    result: Result<WorldStreamUpdate, String>,
}

struct ActiveWorldStream {
    runtime: Arc<Mutex<WorldStreamingRuntime>>,
    report: WorldStreamReport,
    last_stream_position: [f32; 3],
    last_stream_sync: Instant,
    updates_tx: Sender<BackgroundWorldUpdate>,
    updates_rx: Receiver<BackgroundWorldUpdate>,
    update_in_flight: Arc<AtomicBool>,
    wake_sender: Sender<RenderCommand>,
}

enum RenderCommand {
    Load {
        package: Box<RenderPackage>,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    UpdatePackage {
        package: Box<RenderPackage>,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    StartWorldStream {
        source: SceneGameIndexSource,
        config: Box<WorldStreamConfig>,
        position: GtaRpfWorldPoint,
        gpu_budget: GpuCacheBudget,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    WorldStreamAt {
        position: GtaRpfWorldPoint,
        fit_camera: bool,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    WorldSearch {
        query: String,
        limit: usize,
        reply: Sender<Result<GtaRpfBrowserSearchReport, String>>,
    },
    SetWorldOverlays {
        packages: Vec<RenderPackage>,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    ClearWorldOverlays {
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    StopWorldStream {
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    ApplyWorldStreamUpdates,
    Resize {
        width: u32,
        height: u32,
        reply: Option<Sender<Result<NativeViewportReport, String>>>,
    },
    Orbit {
        delta_x: f32,
        delta_y: f32,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Look {
        delta_x: f32,
        delta_y: f32,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Pan {
        delta_x: f32,
        delta_y: f32,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Zoom {
        delta: f32,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Fly {
        forward: f32,
        right: f32,
        up: f32,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    InputFrame {
        look_x: f32,
        look_y: f32,
        pan_x: f32,
        pan_y: f32,
        forward: f32,
        right: f32,
        up: f32,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Pick {
        x: f32,
        y: f32,
        reply: Sender<Result<NativeViewportPickReport, String>>,
    },
    Select {
        node_index: Option<u32>,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    FocusNode {
        node_index: u32,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    SetNodeVisible {
        node_index: u32,
        visible: bool,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    IsolateNode {
        node_index: u32,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    ShowAllNodes {
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    LayerVisibility {
        base_game: bool,
        local_overlays: bool,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Projection {
        projection: Projection,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Overlays {
        grid: bool,
        wireframe: bool,
        bounds: bool,
        collision: bool,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Gizmo {
        mode: Option<TransformGizmoMode>,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Stats {
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    CycleLodDebug {
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    CaptureLodDiagnostic {
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Shutdown,
}

#[tauri::command]
pub(crate) async fn native_viewport_create(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportCreateRequest,
) -> Result<NativeViewportReport, String> {
    if state.smoke_active.load(Ordering::Acquire) {
        return Err("Native viewport is reserved by an active smoke run".into());
    }
    create_native_viewport(app, state, request, NativeViewportWindowMode::Overlay).await
}

#[tauri::command]
pub(crate) async fn native_viewport_create_dedicated(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportCreateRequest,
) -> Result<NativeViewportReport, String> {
    if state.smoke_active.load(Ordering::Acquire) {
        return Err("Native viewport is reserved by an active smoke run".into());
    }
    create_native_viewport(app, state, request, NativeViewportWindowMode::Dedicated).await
}

#[tauri::command]
pub(crate) async fn native_viewport_open_dedicated_scene(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeViewportState>,
    request: WorkspaceSceneRequest,
) -> Result<NativeViewportReport, String> {
    diagnostics::record_event(
        "map",
        "open_scene_requested",
        serde_json::json!({
            "workspace": request.workspace,
            "ymap": request.ymap,
        }),
    );

    if let Err(error) = create_native_viewport(
        app.clone(),
        state,
        NativeViewportCreateRequest {
            width: 1280,
            height: 800,
        },
        NativeViewportWindowMode::DedicatedLoading,
    )
    .await
    {
        diagnostics::record_fatal(
            "map",
            "create_scene_window_failed",
            serde_json::json!({ "error": error }),
        );
        return Err(error);
    }

    let load_started = Instant::now();
    let app_for_load = app.clone();
    let request_for_load = request.clone();
    let loaded = tauri::async_runtime::spawn_blocking(move || {
        load_native_viewport_scene_blocking(&app_for_load, request_for_load)
    })
    .await
    .map_err(|error| error.to_string())?;

    match loaded {
        Ok(report) => {
            native_viewport_focus_window(app.state::<NativeViewportState>())?;
            diagnostics::record_event(
                "map",
                "scene_opened",
                serde_json::json!({
                    "instances": report.stats.instances,
                    "sceneLoadMs": report.stats.scene_load_ms,
                    "openWallMs": load_started.elapsed().as_secs_f64() * 1000.0,
                }),
            );
            Ok(report)
        }
        Err(error) => {
            diagnostics::record_fatal(
                "map",
                "scene_load_failed",
                serde_json::json!({ "error": error }),
            );
            let _ = shutdown_native_viewport(&app.state::<NativeViewportState>());
            Err(error)
        }
    }
}

#[tauri::command]
pub(crate) async fn native_viewport_open_dedicated_world(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeViewportState>,
    request: NativeWorldStreamStartRequest,
) -> Result<NativeViewportReport, String> {
    diagnostics::record_event(
        "map",
        "open_world_requested",
        serde_json::json!({
            "gameRoot": request.game_index.game_root,
            "index": request.game_index.index,
            "position": request.position,
        }),
    );

    if let Err(error) = create_native_viewport(
        app.clone(),
        state,
        NativeViewportCreateRequest {
            width: 1280,
            height: 800,
        },
        NativeViewportWindowMode::DedicatedLoading,
    )
    .await
    {
        diagnostics::record_fatal(
            "map",
            "create_world_window_failed",
            serde_json::json!({ "error": error }),
        );
        return Err(error);
    }

    let load_started = Instant::now();
    let app_for_stream = app.clone();
    let request_for_stream = request.clone();
    let started = tauri::async_runtime::spawn_blocking(move || {
        start_native_world_stream_blocking(&app_for_stream, request_for_stream)
    })
    .await
    .map_err(|error| error.to_string())?;

    match started {
        Ok(report) => {
            native_viewport_focus_window(app.state::<NativeViewportState>())?;
            diagnostics::record_event(
                "map",
                "world_opened",
                serde_json::json!({
                    "activeMaps": report.streaming.as_ref().map(|stream| stream.active_maps),
                    "instances": report.stats.instances,
                    "openWallMs": load_started.elapsed().as_secs_f64() * 1000.0,
                }),
            );
            Ok(report)
        }
        Err(error) => {
            diagnostics::record_fatal(
                "map",
                "world_stream_start_failed",
                serde_json::json!({ "error": error }),
            );
            let _ = shutdown_native_viewport(&app.state::<NativeViewportState>());
            Err(error)
        }
    }
}

async fn create_native_viewport(
    app: tauri::AppHandle,
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportCreateRequest,
    mode: NativeViewportWindowMode,
) -> Result<NativeViewportReport, String> {
    let options = ViewportOptions {
        width: request.width,
        height: request.height,
        ..ViewportOptions::default()
    };
    options.validate().map_err(|error| error.to_string())?;

    if let Some(runtime) = state
        .runtime
        .lock()
        .map_err(|_| "Native viewport state lock is poisoned".to_string())?
        .take()
    {
        runtime.shutdown();
    }

    let generation = NATIVE_VIEWPORT_GENERATION.fetch_add(1, Ordering::Relaxed);
    let window_label = format!("{NATIVE_VIEWPORT_LABEL}-{generation}");
    diagnostics::record_event(
        "renderer",
        "window_create_requested",
        serde_json::json!({
            "label": window_label,
            "mode": format!("{mode:?}"),
            "width": request.width,
            "height": request.height,
        }),
    );
    let builder = tauri::window::WindowBuilder::new(&app, window_label)
        .inner_size(f64::from(request.width), f64::from(request.height));

    let builder = match mode {
        NativeViewportWindowMode::Overlay => {
            let parent = app
                .get_window("main")
                .ok_or_else(|| "RageLab Studio main window is unavailable".to_string())?;
            builder
                .title("RageLab Native Viewport")
                .decorations(false)
                .shadow(false)
                .skip_taskbar(true)
                .resizable(false)
                .focused(false)
                .visible(false)
                .parent(&parent)
                .map_err(core_error)?
        }
        NativeViewportWindowMode::Dedicated => builder
            .title("RageLab Map")
            .center()
            .decorations(true)
            .shadow(true)
            .skip_taskbar(false)
            .resizable(true)
            .focused(true)
            .visible(true),
        NativeViewportWindowMode::DedicatedLoading => builder
            .title("RageLab Map")
            .center()
            .decorations(true)
            .shadow(true)
            .skip_taskbar(false)
            .resizable(true)
            .focused(false)
            .visible(false),
        NativeViewportWindowMode::Standalone => builder
            .title(
                "RageLab Native YMAP Viewer — LMB look · RMB/MMB pan · WASD/QE fly · +/- zoom · Esc close",
            )
            .center()
            .decorations(true)
            .shadow(true)
            .skip_taskbar(false)
            .resizable(true)
            .focused(true)
            .visible(true),
    };

    let window = builder.build().map_err(|error| {
        diagnostics::record_fatal(
            "renderer",
            "window_build_failed",
            serde_json::json!({ "error": error.to_string(), "mode": format!("{mode:?}") }),
        );
        core_error(error)
    })?;
    diagnostics::record_event(
        "renderer",
        "window_created",
        serde_json::json!({ "label": window.label(), "mode": format!("{mode:?}") }),
    );
    match mode {
        NativeViewportWindowMode::Overlay => {
            window.set_ignore_cursor_events(true).map_err(core_error)?;
        }
        NativeViewportWindowMode::Dedicated
        | NativeViewportWindowMode::DedicatedLoading
        | NativeViewportWindowMode::Standalone => {
            window.set_ignore_cursor_events(false).map_err(core_error)?;
            if mode != NativeViewportWindowMode::DedicatedLoading {
                window.set_focus().map_err(core_error)?;
            }
        }
    }

    let (sender, receiver) = mpsc::channel();
    let (ready_sender, ready_receiver) = mpsc::channel();
    let window_alive = Arc::new(AtomicBool::new(true));
    let render_window_alive = window_alive.clone();
    let render_command_sender = sender.clone();
    let render_target = window.clone();
    let render_label = window.label().to_string();
    let thread = thread::Builder::new()
        .name("ragelab-native-viewport".into())
        .spawn(move || {
            diagnostics::record_event(
                "renderer",
                "thread_started",
                serde_json::json!({ "window": render_label }),
            );
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                render_thread(
                    render_target,
                    options,
                    receiver,
                    ready_sender,
                    render_window_alive,
                    render_command_sender,
                );
            }));
            if result.is_err() {
                diagnostics::record_fatal(
                    "renderer",
                    "thread_panicked",
                    serde_json::json!({ "window": render_label }),
                );
            } else {
                diagnostics::record_event(
                    "renderer",
                    "thread_exited",
                    serde_json::json!({ "window": render_label }),
                );
            }
        })
        .map_err(|error| {
            diagnostics::record_fatal(
                "renderer",
                "thread_spawn_failed",
                serde_json::json!({ "error": error.to_string() }),
            );
            core_error(error)
        })?;

    let ready = ready_receiver
        .recv_timeout(RENDER_THREAD_READY_TIMEOUT)
        .map_err(|error| {
            let message = format!("Native viewport renderer did not initialize: {error}");
            diagnostics::record_fatal(
                "renderer",
                "ready_timeout",
                serde_json::json!({ "error": message }),
            );
            message
        })?
        .map_err(|error| {
            diagnostics::record_fatal(
                "renderer",
                "initialization_failed",
                serde_json::json!({ "error": error }),
            );
            error
        })?;
    diagnostics::record_event(
        "renderer",
        "ready",
        serde_json::json!({
            "instances": ready.stats.instances,
            "frameMs": ready.stats.last_frame_ms,
        }),
    );

    let interactive = mode != NativeViewportWindowMode::Overlay;
    let input_stop = interactive.then(|| Arc::new(AtomicBool::new(false)));
    let event_sender = sender.clone();
    let event_app = app.clone();
    let event_input_stop = input_stop.clone();
    let event_window_alive = window_alive.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(size) => {
            let _ = event_sender.send(RenderCommand::Resize {
                width: size.width,
                height: size.height,
                reply: None,
            });
        }
        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
            let _ = event_sender.send(RenderCommand::Resize {
                width: new_inner_size.width,
                height: new_inner_size.height,
                reply: None,
            });
        }
        WindowEvent::Destroyed => {
            event_window_alive.store(false, Ordering::Release);
            diagnostics::record_event(
                "renderer",
                "window_destroyed",
                serde_json::json!({ "mode": format!("{mode:?}") }),
            );
            if let Some(stop) = &event_input_stop {
                stop.store(true, Ordering::Release);
            }
            let _ = event_sender.send(RenderCommand::Shutdown);
            if mode == NativeViewportWindowMode::Standalone {
                event_app.exit(0);
            }
        }
        _ => {}
    });

    let input_thread = if let Some(stop) = input_stop.as_ref() {
        spawn_standalone_input_thread(window.clone(), sender.clone(), stop.clone())?
    } else {
        None
    };

    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "Native viewport state lock is poisoned".to_string())?;
    *runtime = Some(NativeViewportRuntime {
        window,
        sender,
        thread: Some(thread),
        input_stop,
        input_thread,
    });

    Ok(ready)
}

#[cfg(windows)]
fn spawn_standalone_input_thread(
    window: Window,
    sender: Sender<RenderCommand>,
    stop: Arc<AtomicBool>,
) -> Result<Option<JoinHandle<()>>, String> {
    use windows_sys::Win32::{
        Foundation::{HWND, POINT},
        Graphics::Gdi::ScreenToClient,
        UI::{
            Input::KeyboardAndMouse::{
                GetAsyncKeyState, VK_ADD, VK_ESCAPE, VK_F7, VK_F8, VK_LBUTTON, VK_MBUTTON,
                VK_OEM_MINUS, VK_OEM_PLUS, VK_RBUTTON, VK_SHIFT, VK_SUBTRACT,
            },
            WindowsAndMessaging::{GetCursorPos, GetForegroundWindow},
        },
    };

    let hwnd_value = window.hwnd().map_err(core_error)?.0 as usize;
    let input_thread = thread::Builder::new()
        .name("ragelab-native-viewport-input".into())
        .spawn(move || {
            let hwnd = hwnd_value as HWND;
            let key_down = |key: i32| unsafe { (GetAsyncKeyState(key) as u16 & 0x8000) != 0 };
            let mut last_cursor: Option<(i32, i32)> = None;
            let mut left_was_down = false;
            let mut left_dragged = false;
            let mut plus_was_down = false;
            let mut minus_was_down = false;
            let mut escape_was_down = false;
            let mut lod_debug_was_down = false;
            let mut lod_probe_was_down = false;
            let mut lod_debug_step = 0_usize;
            let mut last_motion_sample = Instant::now();

            while !stop.load(Ordering::Acquire) {
                let focused = unsafe { GetForegroundWindow() == hwnd };
                if !focused {
                    last_cursor = None;
                    left_was_down = false;
                    left_dragged = false;
                    last_motion_sample = Instant::now();
                    thread::sleep(Duration::from_millis(16));
                    continue;
                }

                let mut point = POINT { x: 0, y: 0 };
                let cursor_valid = unsafe { GetCursorPos(&mut point) != 0 }
                    && unsafe { ScreenToClient(hwnd, &mut point) != 0 };
                let size = window.inner_size().ok();

                let left_down = key_down(VK_LBUTTON as i32);
                let right_down = key_down(VK_RBUTTON as i32);
                let middle_down = key_down(VK_MBUTTON as i32);
                let mut look_x = 0.0_f32;
                let mut look_y = 0.0_f32;
                let mut pan_x = 0.0_f32;
                let mut pan_y = 0.0_f32;

                if cursor_valid {
                    if let (Some((last_x, last_y)), Some(size)) = (last_cursor, size) {
                        let dx = point.x - last_x;
                        let dy = point.y - last_y;
                        let normalized_x = dx as f32 / size.width.max(1) as f32;
                        let normalized_y = dy as f32 / size.height.max(1) as f32;

                        if left_down && (dx != 0 || dy != 0) {
                            left_dragged |= dx.abs() + dy.abs() > 1;
                            look_x = normalized_x;
                            look_y = normalized_y;
                        } else if (right_down || middle_down) && (dx != 0 || dy != 0) {
                            pan_x = normalized_x;
                            pan_y = normalized_y;
                        }
                    }

                    if left_down && !left_was_down {
                        left_dragged = false;
                    }
                    if !left_down && left_was_down && !left_dragged {
                        if let Some(size) = size {
                            if point.x >= 0
                                && point.y >= 0
                                && point.x < size.width as i32
                                && point.y < size.height as i32
                            {
                                let x = point.x as f32 / size.width.max(1) as f32;
                                let y = point.y as f32 / size.height.max(1) as f32;
                                send_detached_pick(&sender, x, y);
                            }
                        }
                    }
                    last_cursor = Some((point.x, point.y));
                }

                let motion_now = Instant::now();
                let delta_seconds = motion_now
                    .duration_since(last_motion_sample)
                    .as_secs_f32()
                    .clamp(0.001, 0.05);
                last_motion_sample = motion_now;

                let mut forward = 0.0_f32;
                let mut right = 0.0_f32;
                let mut up = 0.0_f32;
                if key_down('W' as i32) {
                    forward += 1.0;
                }
                if key_down('S' as i32) {
                    forward -= 1.0;
                }
                if key_down('D' as i32) {
                    right += 1.0;
                }
                if key_down('A' as i32) {
                    right -= 1.0;
                }
                if key_down('E' as i32) {
                    up += 1.0;
                }
                if key_down('Q' as i32) {
                    up -= 1.0;
                }

                let movement_rate = if key_down(VK_SHIFT as i32) {
                    15.625
                } else {
                    5.0
                };
                let movement_scale = movement_rate * delta_seconds;
                let has_continuous_input = look_x != 0.0
                    || look_y != 0.0
                    || pan_x != 0.0
                    || pan_y != 0.0
                    || forward != 0.0
                    || right != 0.0
                    || up != 0.0;

                if has_continuous_input
                    && !send_paced_input(&sender, |reply| RenderCommand::InputFrame {
                        look_x,
                        look_y,
                        pan_x,
                        pan_y,
                        forward: forward * movement_scale,
                        right: right * movement_scale,
                        up: up * movement_scale,
                        reply,
                    })
                {
                    break;
                }

                let plus_down = key_down(VK_OEM_PLUS as i32) || key_down(VK_ADD as i32);
                if plus_down && !plus_was_down {
                    send_detached(&sender, |reply| RenderCommand::Zoom {
                        delta: -0.75,
                        reply,
                    });
                }
                plus_was_down = plus_down;

                let minus_down = key_down(VK_OEM_MINUS as i32) || key_down(VK_SUBTRACT as i32);
                if minus_down && !minus_was_down {
                    send_detached(&sender, |reply| RenderCommand::Zoom { delta: 0.75, reply });
                }
                minus_was_down = minus_down;

                // F7 compares exactly the same camera in normal, HD-only
                // and LOD-only modes. This is a diagnostic, not the default.
                let lod_debug_down = key_down(VK_F7 as i32);
                if lod_debug_down && !lod_debug_was_down {
                    lod_debug_step = (lod_debug_step + 1) % 3;
                    let modes = ["Normal", "HD + ORPHANHD", "LOD / proxy"];
                    let _ = window.set_title(&format!(
                        "RageLab Map — LOD diagnostic: {} (F7)",
                        modes[lod_debug_step]
                    ));
                    send_detached(&sender, |reply| RenderCommand::CycleLodDebug { reply });
                }
                lod_debug_was_down = lod_debug_down;

                // F8 records all three logical layers in one renderer epoch.
                // It does not cycle F7, move the camera or change residency.
                let lod_probe_down = key_down(VK_F8 as i32);
                if lod_probe_down && !lod_probe_was_down {
                    send_detached(&sender, |reply| RenderCommand::CaptureLodDiagnostic {
                        reply,
                    });
                }
                lod_probe_was_down = lod_probe_down;

                let escape_down = key_down(VK_ESCAPE as i32);
                if escape_down && !escape_was_down {
                    let _ = window.close();
                    break;
                }
                escape_was_down = escape_down;

                left_was_down = left_down;
                thread::sleep(Duration::from_millis(if has_continuous_input {
                    1
                } else {
                    8
                }));
            }
        })
        .map_err(core_error)?;

    Ok(Some(input_thread))
}

#[cfg(not(windows))]
fn spawn_standalone_input_thread(
    _window: Window,
    _sender: Sender<RenderCommand>,
    _stop: Arc<AtomicBool>,
) -> Result<Option<JoinHandle<()>>, String> {
    Ok(None)
}

fn send_detached(
    sender: &Sender<RenderCommand>,
    build: impl FnOnce(Sender<Result<NativeViewportReport, String>>) -> RenderCommand,
) {
    let (reply, _receive) = mpsc::channel();
    let _ = sender.send(build(reply));
}

fn send_paced_input(
    sender: &Sender<RenderCommand>,
    build: impl FnOnce(Sender<Result<NativeViewportReport, String>>) -> RenderCommand,
) -> bool {
    let (reply, receive) = mpsc::channel();
    if sender.send(build(reply)).is_err() {
        return false;
    }
    receive.recv().is_ok()
}

fn send_detached_pick(sender: &Sender<RenderCommand>, x: f32, y: f32) {
    let (reply, _receive) = mpsc::channel();
    let _ = sender.send(RenderCommand::Pick { x, y, reply });
}

#[tauri::command]
pub(crate) async fn native_viewport_load_scene(
    app: tauri::AppHandle,
    request: WorkspaceSceneRequest,
) -> Result<NativeViewportReport, String> {
    tauri::async_runtime::spawn_blocking(move || load_native_viewport_scene_blocking(&app, request))
        .await
        .map_err(|error| error.to_string())?
}

fn load_native_viewport_scene_blocking(
    app: &tauri::AppHandle,
    request: WorkspaceSceneRequest,
) -> Result<NativeViewportReport, String> {
    let package = build_workspace_render_package(&request)?;
    load_render_package(&app.state::<NativeViewportState>(), package)
}

pub(crate) fn load_render_package(
    state: &tauri::State<'_, NativeViewportState>,
    package: RenderPackage,
) -> Result<NativeViewportReport, String> {
    with_runtime(state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::Load {
            package: Box::new(package),
            reply,
        })
    })
}

pub(crate) fn update_render_package(
    state: &tauri::State<'_, NativeViewportState>,
    package: RenderPackage,
) -> Result<NativeViewportReport, String> {
    with_runtime(state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::UpdatePackage {
            package: Box::new(package),
            reply,
        })
    })
}

fn build_workspace_render_package(
    request: &WorkspaceSceneRequest,
) -> Result<RenderPackage, String> {
    let scene = validate_workspace_scene_context(
        &request.workspace,
        &request.ymap,
        &request.fallback_roots,
        &request.rpf_mounts,
        request.game_index.as_ref(),
        request.max_nodes,
    )?;
    workspace_scene_render_package_with_game_index(
        &scene.workspace,
        &scene.ymap,
        SceneAssetPreviewSources {
            fallback_roots: &scene.fallback_roots,
            rpf_mounts: &scene.rpf_mounts,
            game_index: scene.game_index.as_ref(),
        },
        scene.options,
        RenderPackageOptions::default(),
    )
    .map_err(core_error)
}

#[tauri::command]
pub(crate) async fn native_viewport_start_world_stream(
    app: tauri::AppHandle,
    request: NativeWorldStreamStartRequest,
) -> Result<NativeViewportReport, String> {
    tauri::async_runtime::spawn_blocking(move || start_native_world_stream_blocking(&app, request))
        .await
        .map_err(|error| error.to_string())?
}

fn start_native_world_stream_blocking(
    app: &tauri::AppHandle,
    request: NativeWorldStreamStartRequest,
) -> Result<NativeViewportReport, String> {
    let source = validate_world_stream_source(&request.game_index)?;
    let mut config = WorldStreamConfig::default();
    if let Some(value) = request.load_radius {
        config.load_radius = value;
    }
    if let Some(value) = request.retain_radius {
        config.retain_radius = value;
    }
    if let Some(value) = request.max_active_maps {
        config.max_active_maps = value;
    }
    if let Some(value) = request.max_chunk_loads_per_update {
        config.max_chunk_loads_per_update = value;
    }
    if let Some(value) = request.max_cpu_chunks {
        config.max_cpu_chunks = value;
    }
    if let Some(value) = request.max_cpu_bytes {
        config.max_cpu_bytes = value;
    }
    config.validate().map_err(core_error)?;

    let defaults = GpuCacheBudget::default();
    let gpu_budget = GpuCacheBudget {
        max_assets: request.max_gpu_assets.unwrap_or(defaults.max_assets),
        max_textures: request.max_gpu_textures.unwrap_or(defaults.max_textures),
        max_asset_bytes: request
            .max_gpu_asset_bytes
            .unwrap_or(defaults.max_asset_bytes),
        max_texture_bytes: request
            .max_gpu_texture_bytes
            // Camera-adaptive world textures keep 512px only near the
            // camera. Reserve 512 MiB for their mip chains instead of the
            // previous 384 MiB; explicit per-request limits still win.
            .unwrap_or(defaults.max_texture_bytes.saturating_mul(2)),
    };
    gpu_budget.validate().map_err(|error| error.to_string())?;
    let position = world_point(request.position)?;

    with_runtime(&app.state::<NativeViewportState>(), |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::StartWorldStream {
            source,
            config: Box::new(config),
            position,
            gpu_budget,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) async fn native_viewport_world_stream_at(
    app: tauri::AppHandle,
    request: NativeWorldStreamAtRequest,
) -> Result<NativeViewportReport, String> {
    tauri::async_runtime::spawn_blocking(move || stream_native_world_at_blocking(&app, request))
        .await
        .map_err(|error| error.to_string())?
}

fn stream_native_world_at_blocking(
    app: &tauri::AppHandle,
    request: NativeWorldStreamAtRequest,
) -> Result<NativeViewportReport, String> {
    let position = world_point(request.position)?;
    with_runtime(&app.state::<NativeViewportState>(), |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::WorldStreamAt {
            position,
            fit_camera: request.fit_camera,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_world_search(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeWorldSearchRequest,
) -> Result<GtaRpfBrowserSearchReport, String> {
    let query = request.query.trim().to_string();
    if query.is_empty() {
        return Err("World browser search query must not be empty".into());
    }
    let limit = request.limit.unwrap_or(50);
    if !(1..=200).contains(&limit) {
        return Err("World browser search limit must be within 1..=200".into());
    }

    with_runtime(&state, |runtime| {
        let (reply, receive) = mpsc::channel();
        runtime
            .sender
            .send(RenderCommand::WorldSearch {
                query,
                limit,
                reply,
            })
            .map_err(|error| error.to_string())?;
        receive
            .recv_timeout(RENDER_COMMAND_TIMEOUT)
            .map_err(|error| error.to_string())?
    })
}

#[tauri::command]
pub(crate) fn native_viewport_world_set_workspace_overlay(
    state: tauri::State<'_, NativeViewportState>,
    request: WorkspaceSceneRequest,
) -> Result<NativeViewportReport, String> {
    let package = build_workspace_render_package(&request)?;
    set_world_overlay_packages(&state, vec![package])
}

pub(crate) fn set_world_overlay_packages(
    state: &tauri::State<'_, NativeViewportState>,
    packages: Vec<RenderPackage>,
) -> Result<NativeViewportReport, String> {
    with_runtime(state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::SetWorldOverlays {
            packages,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_world_clear_overlays(
    state: tauri::State<'_, NativeViewportState>,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::ClearWorldOverlays {
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_stop_world_stream(
    state: tauri::State<'_, NativeViewportState>,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::StopWorldStream {
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_set_rect(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportRectRequest,
) -> Result<NativeViewportReport, String> {
    if request.width == 0 || request.height == 0 {
        return Err("Native viewport width/height must be greater than zero".into());
    }

    with_runtime(&state, |runtime| {
        runtime
            .window
            .set_position(PhysicalPosition::new(request.x, request.y))
            .map_err(core_error)?;
        runtime
            .window
            .set_size(PhysicalSize::new(request.width, request.height))
            .map_err(core_error)?;
        request_report(&runtime.sender, |reply| RenderCommand::Resize {
            width: request.width,
            height: request.height,
            reply: Some(reply),
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_input(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportInputRequest,
) -> Result<NativeViewportReport, String> {
    if !matches!(
        request.kind.as_str(),
        "orbit" | "look" | "pan" | "zoom" | "fly"
    ) {
        return Err(format!(
            "Unsupported native viewport input kind: {}",
            request.kind
        ));
    }

    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| match request.kind.as_str() {
            "orbit" => RenderCommand::Orbit {
                delta_x: request.delta_x,
                delta_y: request.delta_y,
                reply,
            },
            "look" => RenderCommand::Look {
                delta_x: request.delta_x,
                delta_y: request.delta_y,
                reply,
            },
            "pan" => RenderCommand::Pan {
                delta_x: request.delta_x,
                delta_y: request.delta_y,
                reply,
            },
            "zoom" => RenderCommand::Zoom {
                delta: request.delta,
                reply,
            },
            "fly" => RenderCommand::Fly {
                forward: request.forward,
                right: request.right,
                up: request.up,
                reply,
            },
            _ => unreachable!("native viewport input kind validated above"),
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_pick(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportPickRequest,
) -> Result<NativeViewportPickReport, String> {
    if !(0.0..=1.0).contains(&request.x) || !(0.0..=1.0).contains(&request.y) {
        return Err("Native viewport pick coordinates must be normalized within 0..=1".into());
    }
    with_runtime(&state, |runtime| {
        let (reply, receive) = mpsc::channel();
        runtime
            .sender
            .send(RenderCommand::Pick {
                x: request.x,
                y: request.y,
                reply,
            })
            .map_err(|error| error.to_string())?;
        receive
            .recv_timeout(RENDER_COMMAND_TIMEOUT)
            .map_err(|error| error.to_string())?
    })
}

#[tauri::command]
pub(crate) fn native_viewport_select(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportSelectRequest,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::Select {
            node_index: request.node_index,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_focus_node(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportNodeRequest,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::FocusNode {
            node_index: request.node_index,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_set_node_visible(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportNodeVisibilityRequest,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::SetNodeVisible {
            node_index: request.node_index,
            visible: request.visible,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_isolate_node(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportNodeRequest,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::IsolateNode {
            node_index: request.node_index,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_show_all_nodes(
    state: tauri::State<'_, NativeViewportState>,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::ShowAllNodes {
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_set_layer_visibility(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportLayerVisibilityRequest,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::LayerVisibility {
            base_game: request.base_game,
            local_overlays: request.local_overlays,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_set_projection(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportProjectionRequest,
) -> Result<NativeViewportReport, String> {
    let projection = Projection::parse(&request.projection).ok_or_else(|| {
        "Native viewport projection must be perspective or orthographic".to_string()
    })?;
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::Projection {
            projection,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_set_overlays(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportOverlaysRequest,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::Overlays {
            grid: request.grid,
            wireframe: request.wireframe,
            bounds: request.bounds,
            collision: request.collision,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_set_gizmo(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportGizmoRequest,
) -> Result<NativeViewportReport, String> {
    let mode = request
        .mode
        .as_deref()
        .map(|value| {
            TransformGizmoMode::parse(value).ok_or_else(|| {
                "Native viewport gizmo mode must be translate, rotate, scale or null".to_string()
            })
        })
        .transpose()?;
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::Gizmo {
            mode,
            reply,
        })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_set_visible(
    state: tauri::State<'_, NativeViewportState>,
    request: NativeViewportVisibleRequest,
) -> Result<(), String> {
    with_runtime(&state, |runtime| {
        if request.visible {
            runtime.window.show().map_err(core_error)?;
        } else {
            runtime.window.hide().map_err(core_error)?;
        }
        Ok(())
    })
}

#[tauri::command]
pub(crate) fn native_viewport_focus_window(
    state: tauri::State<'_, NativeViewportState>,
) -> Result<(), String> {
    with_runtime(&state, |runtime| {
        runtime.window.show().map_err(core_error)?;
        runtime.window.set_focus().map_err(core_error)?;
        Ok(())
    })
}

#[tauri::command]
pub(crate) fn native_viewport_stats(
    state: tauri::State<'_, NativeViewportState>,
) -> Result<NativeViewportReport, String> {
    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::Stats { reply })
    })
}

#[tauri::command]
pub(crate) fn native_viewport_shutdown(
    state: tauri::State<'_, NativeViewportState>,
) -> Result<(), String> {
    if state.smoke_active.load(Ordering::Acquire) {
        return Err("Native viewport is reserved by an active smoke run".into());
    }
    shutdown_native_viewport(&state)
}

fn shutdown_native_viewport(state: &NativeViewportState) -> Result<(), String> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|_| "Native viewport state lock is poisoned".to_string())?
        .take();
    if let Some(runtime) = runtime {
        runtime.shutdown();
    }
    Ok(())
}

fn validate_world_stream_source(
    request: &SceneGameIndexRequest,
) -> Result<SceneGameIndexSource, String> {
    let game_root = PathBuf::from(&request.game_root);
    let index = PathBuf::from(&request.index);
    let keys = PathBuf::from(&request.keys);
    if !game_root.is_absolute() || !game_root.is_dir() {
        return Err(format!(
            "Studio world stream game root is invalid: {}",
            game_root.display()
        ));
    }
    if !index.is_absolute() || !index.is_file() {
        return Err(format!(
            "Studio world stream game index is invalid: {}",
            index.display()
        ));
    }
    if !keys.is_absolute() || !keys.is_dir() {
        return Err(format!(
            "Studio world stream key store is invalid: {}",
            keys.display()
        ));
    }
    Ok(SceneGameIndexSource::new(game_root, index, keys))
}

fn world_point(position: [f32; 3]) -> Result<GtaRpfWorldPoint, String> {
    if !position.iter().all(|value| value.is_finite()) {
        return Err("Studio world stream position must be finite".into());
    }
    Ok(GtaRpfWorldPoint {
        x: position[0],
        y: position[1],
        z: position[2],
    })
}

fn with_runtime<T>(
    state: &tauri::State<'_, NativeViewportState>,
    callback: impl FnOnce(&NativeViewportRuntime) -> Result<T, String>,
) -> Result<T, String> {
    let runtime = state
        .runtime
        .lock()
        .map_err(|_| "Native viewport state lock is poisoned".to_string())?;
    let runtime = runtime
        .as_ref()
        .ok_or_else(|| "Native viewport has not been created".to_string())?;
    callback(runtime)
}

fn request_report(
    sender: &Sender<RenderCommand>,
    build: impl FnOnce(Sender<Result<NativeViewportReport, String>>) -> RenderCommand,
) -> Result<NativeViewportReport, String> {
    let (reply, receive) = mpsc::channel();
    sender
        .send(build(reply))
        .map_err(|error| error.to_string())?;
    receive
        .recv_timeout(RENDER_COMMAND_TIMEOUT)
        .map_err(|error| error.to_string())?
}

fn render_thread(
    target: Window,
    options: ViewportOptions,
    receiver: Receiver<RenderCommand>,
    ready: Sender<Result<NativeViewportReport, String>>,
    window_alive: Arc<AtomicBool>,
    command_sender: Sender<RenderCommand>,
) {
    let mut renderer = match SurfaceRenderer::new(target, options) {
        Ok(renderer) => renderer,
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            return;
        }
    };

    let _ = ready.send(Ok(report(&renderer, None)));
    let mut has_scene = false;
    let mut streaming: Option<ActiveWorldStream> = None;

    loop {
        // The render thread must keep presenting and advancing GPU uploads
        // even while there are no user events or fresh world-stream updates.
        let command = if renderer.has_pending_streaming_update() {
            match receiver.recv_timeout(Duration::from_millis(16)) {
                Ok(command) => command,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    if !window_alive.load(Ordering::Acquire) {
                        break;
                    }
                    let pending_before = renderer.pending_streaming_uploads();
                    if let Err(error) = render_if_loaded(&mut renderer, has_scene) {
                        diagnostics::record_event(
                            "world_stream",
                            "incremental_gpu_upload_failed",
                            serde_json::json!({ "error": error.to_string() }),
                        );
                    } else {
                        let remaining = renderer.pending_streaming_uploads();
                        if pending_before > 0
                            && (remaining == 0 || pending_before / 64 != remaining / 64)
                        {
                            let stats = renderer.stats();
                            let (uploaded, elapsed_ms) = renderer.last_streaming_upload_batch();
                            diagnostics::record_event(
                                "world_stream",
                                "incremental_gpu_upload_progress",
                                serde_json::json!({
                                    "pendingBefore": pending_before,
                                    "pendingGpuUploads": remaining,
                                    "uploaded": uploaded,
                                    "gpuUploadBatchMs": elapsed_ms,
                                    "gpuCommitMs": renderer.last_streaming_commit_ms(),
                                    "frameMs": stats.last_frame_ms,
                                    "gpuTextureBytes": stats.gpu_texture_cache_bytes,
                                    "gpuAssetBytes": stats.gpu_asset_cache_bytes,
                                    "gpuBudgetOverflow": stats.gpu_budget_overflow,
                                    "committed": !renderer.has_pending_streaming_update(),
                                }),
                            );
                        }
                    }
                    continue;
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => break,
            }
        } else {
            match receiver.recv() {
                Ok(command) => command,
                Err(_) => break,
            }
        };
        if !window_alive.load(Ordering::Acquire) {
            diagnostics::record_event(
                "renderer",
                "command_loop_cancelled_after_window_destroy",
                serde_json::json!({}),
            );
            break;
        }
        match command {
            RenderCommand::Load { package, reply } => {
                streaming = None;
                let result = renderer
                    .set_package(*package)
                    .and_then(|_| renderer.render_frame())
                    .map(|_| {
                        has_scene = true;
                        report(&renderer, None)
                    })
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
            RenderCommand::UpdatePackage { package, reply } => {
                streaming = None;
                let result = if has_scene {
                    renderer.update_streaming_package(*package)
                } else {
                    renderer.set_package(*package)
                }
                .and_then(|_| renderer.render_frame())
                .map(|_| {
                    has_scene = true;
                    report(&renderer, None)
                })
                .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
            RenderCommand::StartWorldStream {
                source,
                config,
                position,
                gpu_budget,
                reply,
            } => {
                let total_started = Instant::now();
                diagnostics::record_event(
                    "world_stream",
                    "start",
                    serde_json::json!({
                        "position": [position.x, position.y, position.z],
                    }),
                );
                let result = (|| -> Result<NativeViewportReport, String> {
                    renderer
                        .set_gpu_cache_budget(gpu_budget)
                        .map_err(|error| error.to_string())?;

                    let local_camera_radius = WORLD_CAMERA_FOCUS_RADIUS;
                    let open_started = Instant::now();
                    let mut runtime =
                        WorldStreamingRuntime::open(source, *config).map_err(core_error)?;
                    diagnostics::record_event(
                        "world_stream",
                        "runtime_opened",
                        serde_json::json!({
                            "elapsedMs": open_started.elapsed().as_secs_f64() * 1000.0,
                        }),
                    );

                    let query_started = Instant::now();
                    let update = runtime
                        .update(WorldStreamView::at(position))
                        .map_err(core_error)?;
                    diagnostics::record_event(
                        "world_stream",
                        "initial_query_complete",
                        serde_json::json!({
                            "elapsedMs": query_started.elapsed().as_secs_f64() * 1000.0,
                            "activeMaps": update.report.active_maps,
                            "cpuCachedBytes": update.report.cpu_cached_bytes,
                        }),
                    );

                    let packages = update.packages.ok_or_else(|| {
                        "World stream query produced no renderable packages at the requested position"
                            .to_string()
                    })?;
                    if packages.is_empty() {
                        return Err(
                            "World stream query produced no renderable packages at the requested position"
                                .to_string(),
                        );
                    }

                    if !window_alive.load(Ordering::Acquire) {
                        diagnostics::record_event(
                            "world_stream",
                            "initial_load_cancelled_window_closed",
                            serde_json::json!({
                                "totalMs": total_started.elapsed().as_secs_f64() * 1000.0,
                            }),
                        );
                        return Err("Native viewport window closed during world load".into());
                    }

                    let upload_started = Instant::now();
                    renderer.set_overlays(false, false, false);
                    renderer.set_collision_overlay(false);
                    renderer
                        .update_streaming_packages(packages)
                        .map_err(|error| error.to_string())?;
                    renderer.focus_world_position(
                        [position.x, position.y, position.z],
                        local_camera_radius,
                    );
                    renderer.render_frame().map_err(|error| error.to_string())?;
                    diagnostics::record_event(
                        "world_stream",
                        "initial_frame_rendered",
                        serde_json::json!({
                            "elapsedMs": upload_started.elapsed().as_secs_f64() * 1000.0,
                            "totalMs": total_started.elapsed().as_secs_f64() * 1000.0,
                        }),
                    );

                    has_scene = true;
                    let (updates_tx, updates_rx) = mpsc::channel();
                    streaming = Some(ActiveWorldStream {
                        runtime: Arc::new(Mutex::new(runtime)),
                        report: update.report,
                        last_stream_position: [position.x, position.y, position.z],
                        last_stream_sync: Instant::now(),
                        updates_tx,
                        updates_rx,
                        update_in_flight: Arc::new(AtomicBool::new(false)),
                        wake_sender: command_sender.clone(),
                    });
                    schedule_world_stream_to_camera(&renderer, &mut streaming)?;
                    Ok(report(&renderer, streaming.as_ref()))
                })();
                if let Err(error) = &result {
                    if window_alive.load(Ordering::Acquire) {
                        diagnostics::record_fatal(
                            "world_stream",
                            "initial_load_failed",
                            serde_json::json!({
                                "error": error,
                                "totalMs": total_started.elapsed().as_secs_f64() * 1000.0,
                            }),
                        );
                    } else {
                        diagnostics::record_event(
                            "world_stream",
                            "initial_load_aborted",
                            serde_json::json!({
                                "error": error,
                                "totalMs": total_started.elapsed().as_secs_f64() * 1000.0,
                            }),
                        );
                    }
                }
                let _ = reply.send(result);
            }
            RenderCommand::WorldStreamAt {
                position,
                fit_camera,
                reply,
            } => {
                let result = (|| -> Result<NativeViewportReport, String> {
                    let active = streaming
                        .as_mut()
                        .ok_or_else(|| "World streaming is not active".to_string())?;
                    let update = {
                        let mut runtime = active
                            .runtime
                            .lock()
                            .map_err(|_| "World streaming runtime lock is poisoned".to_string())?;
                        runtime
                            .update(WorldStreamView::at(position))
                            .map_err(core_error)?
                    };
                    if let Some(packages) = update.packages {
                        let has_packages = !packages.is_empty();
                        renderer
                            .update_streaming_packages(packages)
                            .map_err(|error| error.to_string())?;
                        if has_packages {
                            if fit_camera {
                                renderer.focus_world_position(
                                    [position.x, position.y, position.z],
                                    WORLD_CAMERA_FOCUS_RADIUS,
                                );
                            }
                            renderer.render_frame().map_err(|error| error.to_string())?;
                        }
                        has_scene = has_packages;
                    }
                    active.report = update.report;
                    active.last_stream_position = [position.x, position.y, position.z];
                    active.last_stream_sync = Instant::now();
                    Ok(report(&renderer, streaming.as_ref()))
                })();
                let _ = reply.send(result);
            }
            RenderCommand::WorldSearch {
                query,
                limit,
                reply,
            } => {
                let result = streaming
                    .as_ref()
                    .ok_or_else(|| "World streaming is not active".to_string())
                    .and_then(|active| {
                        let runtime = active
                            .runtime
                            .lock()
                            .map_err(|_| "World streaming runtime lock is poisoned".to_string())?;
                        runtime
                            .index()
                            .search_browser(&query, limit)
                            .map_err(core_error)
                    });
                let _ = reply.send(result);
            }
            RenderCommand::SetWorldOverlays { packages, reply } => {
                let result = (|| -> Result<NativeViewportReport, String> {
                    {
                        let active = streaming
                            .as_mut()
                            .ok_or_else(|| "World streaming is not active".to_string())?;
                        let update = {
                            let mut runtime = active.runtime.lock().map_err(|_| {
                                "World streaming runtime lock is poisoned".to_string()
                            })?;
                            runtime.set_overlay_packages(packages).map_err(core_error)?;
                            runtime
                                .update(WorldStreamView::at(active.report.position))
                                .map_err(core_error)?
                        };
                        if let Some(packages) = update.packages {
                            let has_packages = !packages.is_empty();
                            renderer
                                .update_streaming_packages(packages)
                                .map_err(|error| error.to_string())?;
                            if has_packages {
                                renderer.render_frame().map_err(|error| error.to_string())?;
                            }
                            has_scene = has_packages;
                        }
                        active.report = update.report;
                    }
                    Ok(report(&renderer, streaming.as_ref()))
                })();
                let _ = reply.send(result);
            }
            RenderCommand::ClearWorldOverlays { reply } => {
                let result = (|| -> Result<NativeViewportReport, String> {
                    {
                        let active = streaming
                            .as_mut()
                            .ok_or_else(|| "World streaming is not active".to_string())?;
                        let update = {
                            let mut runtime = active.runtime.lock().map_err(|_| {
                                "World streaming runtime lock is poisoned".to_string()
                            })?;
                            runtime
                                .set_overlay_packages(Vec::new())
                                .map_err(core_error)?;
                            runtime
                                .update(WorldStreamView::at(active.report.position))
                                .map_err(core_error)?
                        };
                        if let Some(packages) = update.packages {
                            let has_packages = !packages.is_empty();
                            renderer
                                .update_streaming_packages(packages)
                                .map_err(|error| error.to_string())?;
                            if has_packages {
                                renderer.render_frame().map_err(|error| error.to_string())?;
                            }
                            has_scene = has_packages;
                        }
                        active.report = update.report;
                    }
                    Ok(report(&renderer, streaming.as_ref()))
                })();
                let _ = reply.send(result);
            }
            RenderCommand::StopWorldStream { reply } => {
                streaming = None;
                let _ = reply.send(Ok(report(&renderer, None)));
            }
            RenderCommand::ApplyWorldStreamUpdates => {
                if let Err(error) =
                    apply_background_world_updates(&mut renderer, &mut has_scene, &mut streaming)
                {
                    diagnostics::record_event(
                        "world_stream",
                        "background_apply_failed",
                        serde_json::json!({ "error": error }),
                    );
                }
            }
            RenderCommand::Resize {
                width,
                height,
                reply,
            } => {
                let result = renderer
                    .resize(width, height)
                    .and_then(|_| render_if_loaded(&mut renderer, has_scene))
                    .map(|_| report(&renderer, streaming.as_ref()))
                    .map_err(|error| error.to_string());
                if let Some(reply) = reply {
                    let _ = reply.send(result);
                }
            }
            RenderCommand::Orbit {
                delta_x,
                delta_y,
                reply,
            } => {
                renderer.orbit(delta_x, delta_y);
                send_after_input(&mut renderer, &mut has_scene, &mut streaming, reply);
            }
            RenderCommand::Look {
                delta_x,
                delta_y,
                reply,
            } => {
                renderer.look(delta_x, delta_y);
                send_after_input(&mut renderer, &mut has_scene, &mut streaming, reply);
            }
            RenderCommand::Pan {
                delta_x,
                delta_y,
                reply,
            } => {
                renderer.pan(delta_x, delta_y);
                send_after_input(&mut renderer, &mut has_scene, &mut streaming, reply);
            }
            RenderCommand::Zoom { delta, reply } => {
                renderer.zoom(delta);
                send_after_input(&mut renderer, &mut has_scene, &mut streaming, reply);
            }
            RenderCommand::Fly {
                forward,
                right,
                up,
                reply,
            } => {
                renderer.fly(forward, right, up);
                send_after_input(&mut renderer, &mut has_scene, &mut streaming, reply);
            }
            RenderCommand::InputFrame {
                look_x,
                look_y,
                pan_x,
                pan_y,
                forward,
                right,
                up,
                reply,
            } => {
                if look_x != 0.0 || look_y != 0.0 {
                    renderer.look(look_x, look_y);
                }
                if pan_x != 0.0 || pan_y != 0.0 {
                    renderer.pan(pan_x, pan_y);
                }
                if forward != 0.0 || right != 0.0 || up != 0.0 {
                    renderer.fly(forward, right, up);
                }
                send_after_input(&mut renderer, &mut has_scene, &mut streaming, reply);
            }
            RenderCommand::Pick { x, y, reply } => {
                let pick = renderer.pick(x, y);
                renderer.select(pick.map(|value| value.node_index));
                let result = render_if_loaded(&mut renderer, has_scene)
                    .map(|_| NativeViewportPickReport {
                        pick,
                        viewport: report(&renderer, streaming.as_ref()),
                    })
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
            RenderCommand::Select { node_index, reply } => {
                renderer.select(node_index);
                send_after_render(&mut renderer, has_scene, streaming.as_ref(), reply);
            }
            RenderCommand::FocusNode { node_index, reply } => {
                let result = if renderer.focus_node(node_index) {
                    renderer.select(Some(node_index));
                    render_if_loaded(&mut renderer, has_scene)
                        .map(|_| report(&renderer, streaming.as_ref()))
                        .map_err(|error| error.to_string())
                } else {
                    Err(format!(
                        "Native viewport node {node_index} is not visible or does not exist"
                    ))
                };
                let _ = reply.send(result);
            }
            RenderCommand::SetNodeVisible {
                node_index,
                visible,
                reply,
            } => {
                let result = if renderer.set_node_visible(node_index, visible) {
                    render_if_loaded(&mut renderer, has_scene)
                        .map(|_| report(&renderer, streaming.as_ref()))
                        .map_err(|error| error.to_string())
                } else {
                    Err(format!("Native viewport node {node_index} does not exist"))
                };
                let _ = reply.send(result);
            }
            RenderCommand::IsolateNode { node_index, reply } => {
                let result = if renderer.isolate_node(node_index) {
                    renderer.select(Some(node_index));
                    render_if_loaded(&mut renderer, has_scene)
                        .map(|_| report(&renderer, streaming.as_ref()))
                        .map_err(|error| error.to_string())
                } else {
                    Err(format!("Native viewport node {node_index} does not exist"))
                };
                let _ = reply.send(result);
            }
            RenderCommand::ShowAllNodes { reply } => {
                renderer.show_all_nodes();
                send_after_render(&mut renderer, has_scene, streaming.as_ref(), reply);
            }
            RenderCommand::LayerVisibility {
                base_game,
                local_overlays,
                reply,
            } => {
                renderer.set_layer_visibility(base_game, local_overlays);
                send_after_render(&mut renderer, has_scene, streaming.as_ref(), reply);
            }
            RenderCommand::Projection { projection, reply } => {
                renderer.set_projection(projection);
                send_after_render(&mut renderer, has_scene, streaming.as_ref(), reply);
            }
            RenderCommand::Overlays {
                grid,
                wireframe,
                bounds,
                collision,
                reply,
            } => {
                renderer.set_overlays(grid, wireframe, bounds);
                renderer.set_collision_overlay(collision);
                send_after_render(&mut renderer, has_scene, streaming.as_ref(), reply);
            }
            RenderCommand::Gizmo { mode, reply } => {
                renderer.set_gizmo_mode(mode);
                send_after_render(&mut renderer, has_scene, streaming.as_ref(), reply);
            }
            RenderCommand::CycleLodDebug { reply } => {
                let mode = renderer.cycle_lod_debug_mode();
                diagnostics::record_event(
                    "lod",
                    "debug_filter_changed",
                    serde_json::json!({ "mode": mode }),
                );
                send_after_render(&mut renderer, has_scene, streaming.as_ref(), reply);
            }
            RenderCommand::CaptureLodDiagnostic { reply } => {
                let stable = streaming.as_ref().is_some_and(|active| {
                    active.report.pending_chunks == 0
                        && active.report.pending_pages == 0
                        && !active.update_in_flight.load(Ordering::Acquire)
                });
                if stable && has_scene {
                    if let Some(snapshot) = renderer.lod_diagnostic_snapshot(512) {
                        let active = streaming.as_ref().expect("stable requires world stream");
                        diagnostics::record_event(
                            "lod",
                            "fixed_camera_snapshot",
                            serde_json::json!({
                                "schemaVersion": 1,
                                "activeMaps": active.report.active_maps,
                                "pendingChunks": active.report.pending_chunks,
                                "pendingPages": active.report.pending_pages,
                                "maxNodes": 512,
                                "sameCameraAndPackages": true,
                                "snapshot": snapshot,
                            }),
                        );
                    }
                } else {
                    diagnostics::record_event(
                        "lod",
                        "snapshot_skipped_not_settled",
                        serde_json::json!({
                            "hasScene": has_scene,
                            "activeMaps": streaming.as_ref().map(|a| a.report.active_maps),
                            "pendingChunks": streaming.as_ref().map(|a| a.report.pending_chunks),
                            "pendingPages": streaming.as_ref().map(|a| a.report.pending_pages),
                            "updateInFlight": streaming.as_ref().map(|a|
                                a.update_in_flight.load(Ordering::Acquire)),
                        }),
                    );
                }
                let _ = reply.send(Ok(report(&renderer, streaming.as_ref())));
            }
            RenderCommand::Stats { reply } => {
                let _ = reply.send(Ok(report(&renderer, streaming.as_ref())));
            }
            RenderCommand::Shutdown => break,
        }
    }
}

fn send_after_input(
    renderer: &mut SurfaceRenderer,
    has_scene: &mut bool,
    streaming: &mut Option<ActiveWorldStream>,
    reply: Sender<Result<NativeViewportReport, String>>,
) {
    let result = (|| -> Result<NativeViewportReport, String> {
        schedule_world_stream_to_camera(renderer, streaming)?;
        render_if_loaded(renderer, *has_scene).map_err(|error| error.to_string())?;
        Ok(report(renderer, streaming.as_ref()))
    })();
    let _ = reply.send(result);
}

fn schedule_world_stream_to_camera(
    renderer: &SurfaceRenderer,
    streaming: &mut Option<ActiveWorldStream>,
) -> Result<(), String> {
    let Some(active) = streaming.as_mut() else {
        return Ok(());
    };
    if active.update_in_flight.load(Ordering::Acquire) {
        return Ok(());
    }

    let Some(camera) = renderer.camera_snapshot() else {
        return Ok(());
    };
    let position = camera.target;
    let dx = position[0] - active.last_stream_position[0];
    let dy = position[1] - active.last_stream_position[1];
    let horizontal_distance_squared = dx * dx + dy * dy;
    let fill_pending = active.report.pending_chunks > 0
        || active.report.pending_pages > 0
        || active.report.prefetch_pending > 0;

    if !fill_pending {
        if active.last_stream_sync.elapsed() < Duration::from_millis(250) {
            return Ok(());
        }
        if horizontal_distance_squared < 96.0_f32 * 96.0_f32 {
            return Ok(());
        }
    }

    if active
        .update_in_flight
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }

    let point = GtaRpfWorldPoint {
        x: position[0],
        y: position[1],
        z: position[2],
    };
    let runtime = Arc::clone(&active.runtime);
    let updates_tx = active.updates_tx.clone();
    let wake_sender = active.wake_sender.clone();
    active.last_stream_position = position;
    active.last_stream_sync = Instant::now();

    diagnostics::record_event(
        "world_stream",
        "background_update_scheduled",
        serde_json::json!({
            "position": position,
            "pendingChunks": active.report.pending_chunks,
            "pendingPages": active.report.pending_pages,
            "prefetchPending": active.report.prefetch_pending,
        }),
    );

    let spawn_result = thread::Builder::new()
        .name("ragelab-world-stream".into())
        .spawn(move || {
            let started = Instant::now();
            let result = runtime
                .lock()
                .map_err(|_| "World streaming runtime lock is poisoned".to_string())
                .and_then(|mut runtime| {
                    runtime
                        .update(WorldStreamView::at(point))
                        .map_err(core_error)
                });
            let elapsed_ms = started.elapsed().as_secs_f64() * 1000.0;
            let _ = updates_tx.send(BackgroundWorldUpdate {
                position,
                elapsed_ms,
                result,
            });
            let _ = wake_sender.send(RenderCommand::ApplyWorldStreamUpdates);
        });

    if let Err(error) = spawn_result {
        active.update_in_flight.store(false, Ordering::Release);
        return Err(error.to_string());
    }

    Ok(())
}

fn apply_background_world_updates(
    renderer: &mut SurfaceRenderer,
    has_scene: &mut bool,
    streaming: &mut Option<ActiveWorldStream>,
) -> Result<(), String> {
    let mut package_changed = false;
    let mut gpu_apply_ms = None;
    let mut update_completed = false;
    if let Some(active) = streaming.as_mut() {
        while let Ok(background) = active.updates_rx.try_recv() {
            update_completed = true;
            match background.result {
                Ok(update) => {
                    let active_maps = update.report.active_maps;
                    let pending_chunks = update.report.pending_chunks;
                    let prefetch_pending = update.report.prefetch_pending;
                    let loaded_chunks = update.report.loaded_chunks;
                    let prefetched_chunks = update.report.prefetched_chunks;
                    let chunk_loads = update.report.chunk_loads.clone();
                    if let Some(packages) = update.packages {
                        let gpu_started = Instant::now();
                        let has_packages = !packages.is_empty();
                        renderer
                            .queue_streaming_packages(packages)
                            .map_err(|error| error.to_string())?;
                        gpu_apply_ms = Some(gpu_started.elapsed().as_secs_f64() * 1000.0);
                        *has_scene = has_packages;
                        package_changed = has_packages;
                    }
                    active.report = update.report;
                    diagnostics::record_event(
                        "world_stream",
                        "background_update_applied",
                        serde_json::json!({
                            "position": background.position,
                            "elapsedMs": background.elapsed_ms,
                            "gpuApplyMs": gpu_apply_ms,
                            "pendingGpuUploads": renderer.pending_streaming_uploads(),
                            "gpuBudgetOverflow": renderer.stats().gpu_budget_overflow,
                            "activeMaps": active_maps,
                            "loadedChunks": loaded_chunks,
                            "prefetchedChunks": prefetched_chunks,
                            "chunkLoads": chunk_loads,
                            "pendingChunks": pending_chunks,
                            "pendingPages": active.report.pending_pages,
                            "compiledPages": active.report.compiled_pages,
                            "failedPages": active.report.failed_pages,
                            "hdLodInstances": active.report.hd_lod_instances,
                            "proxyLodInstances": active.report.proxy_lod_instances,
                            "unknownLodInstances": active.report.unknown_lod_instances,
                            "spatialOmittedNodes": active.report.spatial_omitted_nodes,
                            "prefetchPending": prefetch_pending,
                        }),
                    );
                }
                Err(error) => {
                    diagnostics::record_event(
                        "world_stream",
                        "background_update_failed",
                        serde_json::json!({
                            "position": background.position,
                            "elapsedMs": background.elapsed_ms,
                            "error": error,
                        }),
                    );
                }
            }
        }
        if update_completed {
            active.update_in_flight.store(false, Ordering::Release);
        }
    }

    if package_changed {
        renderer.render_frame().map_err(|error| error.to_string())?;
        let stats = renderer.stats();
        diagnostics::record_event(
            "world_stream",
            "background_frame_rendered",
            serde_json::json!({
                "instances": stats.instances,
                "visibleInstances": stats.visible_instances,
                "frameMs": stats.last_frame_ms,
                "gpuAssetBytes": stats.gpu_asset_cache_bytes,
                "gpuTextureBytes": stats.gpu_texture_cache_bytes,
                "gpuBudgetOverflow": stats.gpu_budget_overflow,
                "gpuApplyMs": gpu_apply_ms,
                "pendingGpuUploads": renderer.pending_streaming_uploads(),
                "gpuUploadBatch": renderer.last_streaming_upload_batch().0,
                "gpuUploadBatchMs": renderer.last_streaming_upload_batch().1,
                "sceneLoadMs": stats.scene_load_ms,
            }),
        );
    }

    schedule_world_stream_to_camera(renderer, streaming)
}

fn send_after_render(
    renderer: &mut SurfaceRenderer,
    has_scene: bool,
    streaming: Option<&ActiveWorldStream>,
    reply: Sender<Result<NativeViewportReport, String>>,
) {
    let result = render_if_loaded(renderer, has_scene)
        .map(|_| report(renderer, streaming))
        .map_err(|error| error.to_string());
    let _ = reply.send(result);
}

fn render_if_loaded(
    renderer: &mut SurfaceRenderer,
    has_scene: bool,
) -> ragelab_render::RenderResult<()> {
    if has_scene {
        renderer.render_frame()
    } else {
        Ok(())
    }
}

fn report(
    renderer: &SurfaceRenderer,
    streaming: Option<&ActiveWorldStream>,
) -> NativeViewportReport {
    NativeViewportReport {
        stats: renderer.stats(),
        camera: renderer.camera_snapshot(),
        streaming: streaming.map(|active| active.report.clone()),
    }
}

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeViewportOpenSpec {
    scene: WorkspaceSceneRequest,
    #[serde(default = "smoke_width")]
    width: u32,
    #[serde(default = "smoke_height")]
    height: u32,
}

#[cfg(debug_assertions)]
pub(crate) fn maybe_start_debug_open(app: tauri::AppHandle) {
    let Some(spec_path) = debug_cli_arg_path("--native-viewport-open") else {
        return;
    };

    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_debug_open(&app, &spec_path).await {
            eprintln!("native viewport open failed: {error}");
            app.exit(1);
        }
    });
}

#[cfg(debug_assertions)]
async fn run_debug_open(app: &tauri::AppHandle, spec_path: &str) -> Result<(), String> {
    let bytes = std::fs::read(spec_path).map_err(core_error)?;
    let spec: NativeViewportOpenSpec = serde_json::from_slice(&bytes).map_err(core_error)?;
    if let Some(main) = app.get_window("main") {
        main.hide().map_err(core_error)?;
    }

    create_native_viewport(
        app.clone(),
        app.state::<NativeViewportState>(),
        NativeViewportCreateRequest {
            width: spec.width,
            height: spec.height,
        },
        NativeViewportWindowMode::Standalone,
    )
    .await?;

    let report = load_native_viewport_scene_blocking(app, spec.scene.clone())?;
    eprintln!(
        "native viewport open: {} instances, {} assets, {} meshes, {} textures, {:.2} ms load",
        report.stats.instances,
        report.stats.assets,
        report.stats.meshes,
        report.stats.textures,
        report.stats.scene_load_ms
    );
    Ok(())
}

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeWorldOpenSpec {
    game_index: SceneGameIndexRequest,
    position: [f32; 3],
    #[serde(default = "smoke_width")]
    width: u32,
    #[serde(default = "smoke_height")]
    height: u32,
    load_radius: Option<f32>,
    retain_radius: Option<f32>,
    max_active_maps: Option<usize>,
    max_cpu_chunks: Option<usize>,
    max_cpu_bytes: Option<u64>,
}

#[cfg(debug_assertions)]
pub(crate) fn maybe_start_debug_world_open(app: tauri::AppHandle) {
    let Some(spec_path) = debug_cli_arg_path("--native-world-open") else {
        return;
    };

    tauri::async_runtime::spawn(async move {
        if let Err(error) = run_debug_world_open(&app, &spec_path).await {
            eprintln!("native world open failed: {error}");
            app.exit(1);
        }
    });
}

#[cfg(debug_assertions)]
async fn run_debug_world_open(app: &tauri::AppHandle, spec_path: &str) -> Result<(), String> {
    let bytes = std::fs::read(spec_path).map_err(core_error)?;
    let spec: NativeWorldOpenSpec = serde_json::from_slice(&bytes).map_err(core_error)?;

    if let Some(main) = app.get_window("main") {
        main.hide().map_err(core_error)?;
    }

    create_native_viewport(
        app.clone(),
        app.state::<NativeViewportState>(),
        NativeViewportCreateRequest {
            width: spec.width,
            height: spec.height,
        },
        NativeViewportWindowMode::DedicatedLoading,
    )
    .await?;

    let report = start_native_world_stream_blocking(
        app,
        NativeWorldStreamStartRequest {
            game_index: spec.game_index,
            position: spec.position,
            load_radius: spec.load_radius.or(Some(250.0)),
            retain_radius: spec.retain_radius.or(Some(350.0)),
            max_active_maps: spec.max_active_maps.or(Some(4)),
            max_chunk_loads_per_update: Some(2),
            max_cpu_chunks: spec.max_cpu_chunks.or(Some(24)),
            max_cpu_bytes: spec.max_cpu_bytes,
            max_gpu_assets: None,
            max_gpu_textures: None,
            max_gpu_asset_bytes: None,
            max_gpu_texture_bytes: None,
        },
    )?;

    native_viewport_focus_window(app.state::<NativeViewportState>())?;
    diagnostics::record_event(
        "map",
        "debug_world_opened",
        serde_json::json!({
            "activeMaps": report.streaming.as_ref().map(|stream| stream.active_maps),
            "instances": report.stats.instances,
        }),
    );
    eprintln!(
        "native world open: {} instances, {} active maps",
        report.stats.instances,
        report
            .streaming
            .as_ref()
            .map(|stream| stream.active_maps)
            .unwrap_or(0)
    );
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeViewportSmokeSpec {
    output: String,
    scene: WorkspaceSceneRequest,
    #[serde(default = "smoke_width")]
    width: u32,
    #[serde(default = "smoke_height")]
    height: u32,
    #[serde(default)]
    collision: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeViewportSmokeOutput {
    ok: bool,
    core_revision: &'static str,
    created: Option<NativeViewportReport>,
    loaded: Option<NativeViewportReport>,
    load_wall_ms: f64,
    benchmark_frames: u32,
    benchmark_frame_wall_ms: f64,
    collision_overlay: bool,
    pick: Option<NativeViewportPickReport>,
    final_report: Option<NativeViewportReport>,
    error: Option<String>,
}

const fn smoke_width() -> u32 {
    960
}

const fn smoke_height() -> u32 {
    540
}

pub(crate) fn maybe_start_viewport_smoke(app: tauri::AppHandle, core_revision: &'static str) {
    let spec_path = std::env::var("RAGELAB_NATIVE_VIEWPORT_SMOKE")
        .ok()
        .or_else(|| {
            let mut args = std::env::args();
            while let Some(arg) = args.next() {
                if arg == "--native-viewport-smoke" {
                    return args.next();
                }
                if let Some(path) = arg.strip_prefix("--native-viewport-smoke=") {
                    return Some(path.to_string());
                }
            }
            None
        });
    let Some(spec_path) = spec_path else {
        return;
    };

    app.state::<NativeViewportState>()
        .smoke_active
        .store(true, Ordering::Release);
    tauri::async_runtime::spawn(async move {
        let result = run_viewport_smoke(&app, &spec_path, core_revision).await;
        app.state::<NativeViewportState>()
            .smoke_active
            .store(false, Ordering::Release);
        let exit_code = if result { 0 } else { 1 };
        app.exit(exit_code);
    });
}

async fn run_viewport_smoke(
    app: &tauri::AppHandle,
    spec_path: &str,
    core_revision: &'static str,
) -> bool {
    let spec_bytes = match std::fs::read(spec_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("native viewport smoke: unable to read spec {spec_path}: {error}");
            return false;
        }
    };
    let spec: NativeViewportSmokeSpec = match serde_json::from_slice(&spec_bytes) {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("native viewport smoke: invalid spec {spec_path}: {error}");
            return false;
        }
    };

    let mut output = NativeViewportSmokeOutput {
        ok: false,
        core_revision,
        created: None,
        loaded: None,
        load_wall_ms: 0.0,
        benchmark_frames: 0,
        benchmark_frame_wall_ms: 0.0,
        collision_overlay: spec.collision,
        pick: None,
        final_report: None,
        error: None,
    };

    let smoke_result: Result<(), String> = async {
        let created = create_native_viewport(
            app.clone(),
            app.state::<NativeViewportState>(),
            NativeViewportCreateRequest {
                width: spec.width,
                height: spec.height,
            },
            NativeViewportWindowMode::Overlay,
        )
        .await?;
        output.created = Some(created);

        native_viewport_set_visible(
            app.state::<NativeViewportState>(),
            NativeViewportVisibleRequest { visible: true },
        )?;

        let load_started = Instant::now();
        let loaded = load_native_viewport_scene_blocking(app, spec.scene.clone())?;
        output.load_wall_ms = load_started.elapsed().as_secs_f64() * 1000.0;
        output.loaded = Some(loaded);

        native_viewport_set_overlays(
            app.state::<NativeViewportState>(),
            NativeViewportOverlaysRequest {
                grid: true,
                wireframe: false,
                bounds: false,
                collision: spec.collision,
            },
        )?;

        const BENCHMARK_FRAMES: u32 = 90;
        let frame_benchmark_started = Instant::now();
        for _ in 0..BENCHMARK_FRAMES {
            native_viewport_input(
                app.state::<NativeViewportState>(),
                NativeViewportInputRequest {
                    kind: "orbit".into(),
                    delta_x: 0.0,
                    delta_y: 0.0,
                    delta: 0.0,
                    forward: 0.0,
                    right: 0.0,
                    up: 0.0,
                },
            )?;
        }
        output.benchmark_frames = BENCHMARK_FRAMES;
        output.benchmark_frame_wall_ms =
            frame_benchmark_started.elapsed().as_secs_f64() * 1000.0 / f64::from(BENCHMARK_FRAMES);

        native_viewport_input(
            app.state::<NativeViewportState>(),
            NativeViewportInputRequest {
                kind: "orbit".into(),
                delta_x: 0.04,
                delta_y: -0.025,
                delta: 0.0,
                forward: 0.0,
                right: 0.0,
                up: 0.0,
            },
        )?;
        native_viewport_input(
            app.state::<NativeViewportState>(),
            NativeViewportInputRequest {
                kind: "zoom".into(),
                delta_x: 0.0,
                delta_y: 0.0,
                delta: -0.2,
                forward: 0.0,
                right: 0.0,
                up: 0.0,
            },
        )?;

        let pick = native_viewport_pick(
            app.state::<NativeViewportState>(),
            NativeViewportPickRequest { x: 0.5, y: 0.5 },
        )?;
        output.pick = Some(pick);

        let final_report = native_viewport_stats(app.state::<NativeViewportState>())?;
        output.final_report = Some(final_report);
        shutdown_native_viewport(&app.state::<NativeViewportState>())?;
        Ok(())
    }
    .await;

    match smoke_result {
        Ok(()) => output.ok = true,
        Err(error) => {
            output.error = Some(error);
            let _ = shutdown_native_viewport(&app.state::<NativeViewportState>());
        }
    }

    let write_result = serde_json::to_vec_pretty(&output)
        .map_err(|error| error.to_string())
        .and_then(|bytes| std::fs::write(&spec.output, bytes).map_err(|error| error.to_string()));
    if let Err(error) = write_result {
        eprintln!(
            "native viewport smoke: unable to write report {}: {error}",
            spec.output
        );
        return false;
    }

    output.ok
}

#[cfg(debug_assertions)]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeWorldStreamSmokePoint {
    name: String,
    position: [f32; 3],
}

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeWorldStreamSmokeSpec {
    output: String,
    game_index: SceneGameIndexRequest,
    points: Vec<NativeWorldStreamSmokePoint>,
    #[serde(default = "smoke_width")]
    width: u32,
    #[serde(default = "smoke_height")]
    height: u32,
    load_radius: Option<f32>,
    retain_radius: Option<f32>,
    max_active_maps: Option<usize>,
    max_cpu_chunks: Option<usize>,
    max_cpu_bytes: Option<u64>,
    #[serde(default)]
    browser_validation: bool,
    #[serde(default)]
    workspace_overlay: Option<WorkspaceSceneRequest>,
}

#[cfg(debug_assertions)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeWorldStreamSmokeStep {
    name: String,
    position: [f32; 3],
    wall_ms: f64,
    report: NativeViewportReport,
}

#[cfg(debug_assertions)]
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct NativeWorldStreamSmokeOutput {
    ok: bool,
    core_revision: &'static str,
    steps: Vec<NativeWorldStreamSmokeStep>,
    max_active_maps_seen: usize,
    max_cpu_cached_bytes: u64,
    max_gpu_asset_cache_bytes: u64,
    max_gpu_texture_cache_bytes: u64,
    revisit_cache_hit: bool,
    browser_search: Option<GtaRpfBrowserSearchReport>,
    browser_node_index: Option<u32>,
    provider_asset_found: bool,
    overlay_mounted: bool,
    overlay_cleared: bool,
    error: Option<String>,
}

#[cfg(debug_assertions)]
pub(crate) fn maybe_start_debug_world_stream_smoke(
    app: tauri::AppHandle,
    core_revision: &'static str,
) {
    let Some(spec_path) = debug_cli_arg_path("--native-world-stream-smoke") else {
        return;
    };

    app.state::<NativeViewportState>()
        .smoke_active
        .store(true, Ordering::Release);
    tauri::async_runtime::spawn(async move {
        let result = run_debug_world_stream_smoke(&app, &spec_path, core_revision).await;
        app.state::<NativeViewportState>()
            .smoke_active
            .store(false, Ordering::Release);
        app.exit(if result { 0 } else { 1 });
    });
}

#[cfg(debug_assertions)]
async fn run_debug_world_stream_smoke(
    app: &tauri::AppHandle,
    spec_path: &str,
    core_revision: &'static str,
) -> bool {
    let spec_bytes = match std::fs::read(spec_path) {
        Ok(bytes) => bytes,
        Err(error) => {
            eprintln!("native world stream smoke: unable to read spec {spec_path}: {error}");
            return false;
        }
    };
    let spec: NativeWorldStreamSmokeSpec = match serde_json::from_slice(&spec_bytes) {
        Ok(spec) => spec,
        Err(error) => {
            eprintln!("native world stream smoke: invalid spec {spec_path}: {error}");
            return false;
        }
    };

    let mut output = NativeWorldStreamSmokeOutput {
        ok: false,
        core_revision,
        steps: Vec::new(),
        max_active_maps_seen: 0,
        max_cpu_cached_bytes: 0,
        max_gpu_asset_cache_bytes: 0,
        max_gpu_texture_cache_bytes: 0,
        revisit_cache_hit: false,
        browser_search: None,
        browser_node_index: None,
        provider_asset_found: false,
        overlay_mounted: false,
        overlay_cleared: false,
        error: None,
    };

    let smoke_result: Result<(), String> = async {
        if spec.points.len() < 2 {
            return Err("world stream smoke requires at least two points".into());
        }

        create_native_viewport(
            app.clone(),
            app.state::<NativeViewportState>(),
            NativeViewportCreateRequest {
                width: spec.width,
                height: spec.height,
            },
            NativeViewportWindowMode::DedicatedLoading,
        )
        .await?;

        let first = &spec.points[0];
        let max_active_maps = spec.max_active_maps.unwrap_or(4);
        let started_at = Instant::now();
        let first_report = start_native_world_stream_blocking(
            app,
            NativeWorldStreamStartRequest {
                game_index: spec.game_index.clone(),
                position: first.position,
                load_radius: spec.load_radius.or(Some(300.0)),
                retain_radius: spec.retain_radius.or(Some(450.0)),
                max_active_maps: Some(max_active_maps),
                max_chunk_loads_per_update: Some(2),
                max_cpu_chunks: spec.max_cpu_chunks.or(Some(16)),
                max_cpu_bytes: spec.max_cpu_bytes.or(Some(768 * 1024 * 1024)),
                max_gpu_assets: None,
                max_gpu_textures: None,
                max_gpu_asset_bytes: None,
                max_gpu_texture_bytes: None,
            },
        )?;
        let first_wall_ms = started_at.elapsed().as_secs_f64() * 1000.0;
        native_viewport_focus_window(app.state::<NativeViewportState>())?;
        validate_world_stream_smoke_report(&first_report, max_active_maps)?;
        record_world_stream_smoke_step(
            &mut output,
            first.name.clone(),
            first.position,
            first_wall_ms,
            first_report,
        );

        for point in spec.points.iter().skip(1) {
            let started_at = Instant::now();
            let report = stream_native_world_at_blocking(
                app,
                NativeWorldStreamAtRequest {
                    position: point.position,
                    fit_camera: true,
                },
            )?;
            let wall_ms = started_at.elapsed().as_secs_f64() * 1000.0;
            validate_world_stream_smoke_report(&report, max_active_maps)?;
            record_world_stream_smoke_step(
                &mut output,
                point.name.clone(),
                point.position,
                wall_ms,
                report,
            );
        }

        if spec.points.last().map(|point| point.position) != Some(first.position) {
            return Err("world stream smoke last point must revisit the first position".into());
        }
        let revisit = output
            .steps
            .last()
            .and_then(|step| step.report.streaming.as_ref())
            .ok_or_else(|| "world stream smoke revisit has no streaming report".to_string())?;
        output.revisit_cache_hit = revisit.cache_hits > 0;
        if !output.revisit_cache_hit {
            return Err("world stream smoke revisit produced zero CPU chunk cache hits".into());
        }

        if spec.browser_validation {
            validate_native_world_browser_smoke(app, &spec, &mut output)?;
        }

        // The controller exits the app after this function returns.
        // Closing the last viewport here could terminate the app before the
        // smoke report below is persisted to disk.
        Ok(())
    }
    .await;

    match smoke_result {
        Ok(()) => output.ok = true,
        Err(error) => output.error = Some(error),
    }

    let write_result = serde_json::to_vec_pretty(&output)
        .map_err(|error| error.to_string())
        .and_then(|bytes| std::fs::write(&spec.output, bytes).map_err(|error| error.to_string()));
    if let Err(error) = write_result {
        eprintln!(
            "native world stream smoke: unable to write report {}: {error}",
            spec.output
        );
        return false;
    }

    output.ok
}

#[cfg(debug_assertions)]
fn validate_native_world_browser_smoke(
    app: &tauri::AppHandle,
    spec: &NativeWorldStreamSmokeSpec,
    output: &mut NativeWorldStreamSmokeOutput,
) -> Result<(), String> {
    let (node_index, archetype_hash) = {
        let streaming = output
            .steps
            .last()
            .and_then(|step| step.report.streaming.as_ref())
            .ok_or_else(|| "world browser smoke has no active streaming report".to_string())?;
        streaming
            .active_chunks
            .iter()
            .flat_map(|map| &map.entities)
            .find_map(|entry| {
                let node_index = entry.render_node_index?;
                let resolution = entry.resolution.as_ref()?;
                resolution.asset_provider.as_ref()?;
                Some((node_index, entry.entity.archetype_hash))
            })
            .ok_or_else(|| {
                "world browser smoke found no rendered entity with a resolved provider asset"
                    .to_string()
            })?
    };

    let query = format!("0x{archetype_hash:08X}");
    let search = native_viewport_world_search(
        app.state::<NativeViewportState>(),
        NativeWorldSearchRequest {
            query,
            limit: Some(20),
        },
    )?;
    let provider_asset_found = search
        .results
        .iter()
        .any(|result| result.hash == archetype_hash && result.asset_provider.is_some());
    if !provider_asset_found {
        return Err(
            "world browser smoke exact archetype search did not resolve a provider asset".into(),
        );
    }
    output.browser_search = Some(search);
    output.browser_node_index = Some(node_index);
    output.provider_asset_found = true;

    native_viewport_focus_node(
        app.state::<NativeViewportState>(),
        NativeViewportNodeRequest { node_index },
    )?;
    native_viewport_isolate_node(
        app.state::<NativeViewportState>(),
        NativeViewportNodeRequest { node_index },
    )?;
    native_viewport_show_all_nodes(app.state::<NativeViewportState>())?;
    native_viewport_set_node_visible(
        app.state::<NativeViewportState>(),
        NativeViewportNodeVisibilityRequest {
            node_index,
            visible: false,
        },
    )?;
    native_viewport_set_node_visible(
        app.state::<NativeViewportState>(),
        NativeViewportNodeVisibilityRequest {
            node_index,
            visible: true,
        },
    )?;

    if let Some(overlay) = spec.workspace_overlay.as_ref() {
        let overlay_package = build_workspace_render_package(overlay)?;
        let overlay_archetype = overlay_package
            .descriptor
            .scene
            .instances
            .first()
            .map(|instance| instance.archetype_hash)
            .ok_or_else(|| "world browser overlay has no instances".to_string())?;

        let mounted = native_viewport_world_set_workspace_overlay(
            app.state::<NativeViewportState>(),
            overlay.clone(),
        )?;
        output.overlay_mounted = mounted
            .streaming
            .as_ref()
            .is_some_and(|streaming| streaming.overlay_packages == 1);
        if !output.overlay_mounted {
            return Err("world browser smoke failed to mount exactly one workspace overlay".into());
        }

        native_viewport_set_layer_visibility(
            app.state::<NativeViewportState>(),
            NativeViewportLayerVisibilityRequest {
                base_game: false,
                local_overlays: true,
            },
        )?;
        native_viewport_set_layer_visibility(
            app.state::<NativeViewportState>(),
            NativeViewportLayerVisibilityRequest {
                base_game: true,
                local_overlays: true,
            },
        )?;

        let overlay_search = native_viewport_world_search(
            app.state::<NativeViewportState>(),
            NativeWorldSearchRequest {
                query: format!("0x{overlay_archetype:08X}"),
                limit: Some(20),
            },
        )?;
        let overlay_provider_found = overlay_search
            .results
            .iter()
            .any(|result| result.hash == overlay_archetype && result.asset_provider.is_some());
        if !overlay_provider_found {
            return Err(format!(
                "world browser smoke overlay archetype 0x{overlay_archetype:08X} did not resolve back to a GTA provider asset"
            ));
        }
        output.browser_search = Some(overlay_search);
        output.provider_asset_found = true;

        let cleared = native_viewport_world_clear_overlays(app.state::<NativeViewportState>())?;
        output.overlay_cleared = cleared
            .streaming
            .as_ref()
            .is_some_and(|streaming| streaming.overlay_packages == 0);
        if !output.overlay_cleared {
            return Err("world browser smoke failed to clear workspace overlays".into());
        }
    }

    Ok(())
}

#[cfg(debug_assertions)]
fn validate_world_stream_smoke_report(
    report: &NativeViewportReport,
    max_active_maps: usize,
) -> Result<(), String> {
    let streaming = report
        .streaming
        .as_ref()
        .ok_or_else(|| "world stream smoke report has no streaming diagnostics".to_string())?;
    if streaming.active_maps == 0 {
        return Err("world stream smoke loaded zero active maps".into());
    }
    if streaming.active_maps > max_active_maps {
        return Err(format!(
            "world stream smoke exceeded active-map budget: {} > {}",
            streaming.active_maps, max_active_maps
        ));
    }
    if streaming.cpu_budget_overflow || report.stats.gpu_budget_overflow {
        return Err("world stream smoke exceeded CPU/GPU residency budget".into());
    }
    if !streaming.load_errors.is_empty() {
        return Err(format!(
            "world stream smoke has {} chunk load error(s)",
            streaming.load_errors.len()
        ));
    }
    Ok(())
}

#[cfg(debug_assertions)]
fn record_world_stream_smoke_step(
    output: &mut NativeWorldStreamSmokeOutput,
    name: String,
    position: [f32; 3],
    wall_ms: f64,
    report: NativeViewportReport,
) {
    if let Some(streaming) = report.streaming.as_ref() {
        output.max_active_maps_seen = output.max_active_maps_seen.max(streaming.active_maps);
        output.max_cpu_cached_bytes = output.max_cpu_cached_bytes.max(streaming.cpu_cached_bytes);
    }
    output.max_gpu_asset_cache_bytes = output
        .max_gpu_asset_cache_bytes
        .max(report.stats.gpu_asset_cache_bytes);
    output.max_gpu_texture_cache_bytes = output
        .max_gpu_texture_cache_bytes
        .max(report.stats.gpu_texture_cache_bytes);
    output.steps.push(NativeWorldStreamSmokeStep {
        name,
        position,
        wall_ms,
        report,
    });
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct DebugThreeViewportBenchmarkSpec {
    output: String,
    workspace: String,
    ymap: String,
    gta_legacy_root: String,
    max_nodes: Option<usize>,
}

#[tauri::command]
pub(crate) fn debug_three_viewport_benchmark_spec(
) -> Result<Option<DebugThreeViewportBenchmarkSpec>, String> {
    #[cfg(debug_assertions)]
    {
        let Some(path) = debug_cli_arg_path("--three-viewport-benchmark") else {
            return Ok(None);
        };
        let bytes = std::fs::read(&path).map_err(core_error)?;
        let spec = serde_json::from_slice(&bytes).map_err(core_error)?;
        Ok(Some(spec))
    }

    #[cfg(not(debug_assertions))]
    {
        Ok(None)
    }
}

#[tauri::command]
pub(crate) fn debug_three_viewport_benchmark_complete(
    app: tauri::AppHandle,
    report: serde_json::Value,
) -> Result<(), String> {
    #[cfg(debug_assertions)]
    {
        let path = debug_cli_arg_path("--three-viewport-benchmark")
            .ok_or_else(|| "Three viewport benchmark mode is not active".to_string())?;
        let bytes = std::fs::read(&path).map_err(core_error)?;
        let spec: DebugThreeViewportBenchmarkSpec =
            serde_json::from_slice(&bytes).map_err(core_error)?;
        let output = serde_json::json!({
            "schema": "ragelab.studio.viewport-benchmark",
            "schemaVersion": 1,
            "backend": "threejs",
            "coreRevision": super::RAGELAB_CORE_REVISION,
            "report": report,
        });
        let output_bytes = serde_json::to_vec_pretty(&output).map_err(core_error)?;
        std::fs::write(&spec.output, output_bytes).map_err(core_error)?;
        app.exit(0);
        Ok(())
    }

    #[cfg(not(debug_assertions))]
    {
        let _ = (app, report);
        Err("Three viewport benchmark is only available in debug builds".into())
    }
}

#[cfg(debug_assertions)]
fn debug_cli_arg_path(flag: &str) -> Option<String> {
    let prefix = format!("{flag}=");
    let mut args = std::env::args();
    while let Some(arg) = args.next() {
        if arg == flag {
            return args.next();
        }
        if let Some(path) = arg.strip_prefix(&prefix) {
            return Some(path.to_string());
        }
    }
    None
}

fn core_error(error: impl std::fmt::Display) -> String {
    error.to_string()
}
