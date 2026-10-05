use std::{
    sync::{
        mpsc::{self, Receiver, Sender},
        Mutex,
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use ragelab_engine::{
    workspace_scene_render_package_with_game_index, RenderPackage, RenderPackageOptions,
    SceneAssetPreviewSources,
};
use ragelab_render::{
    CameraSnapshot, PickResult, Projection, SurfaceRenderer, ViewportOptions, ViewportStats,
};
use serde::{Deserialize, Serialize};
use tauri::{Manager, PhysicalPosition, PhysicalSize, Window, WindowEvent};

use super::{validate_workspace_scene_context, WorkspaceSceneRequest};

const NATIVE_VIEWPORT_LABEL: &str = "native-viewport";
const RENDER_THREAD_READY_TIMEOUT: Duration = Duration::from_secs(30);
const RENDER_COMMAND_TIMEOUT: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(crate) struct NativeViewportState {
    runtime: Mutex<Option<NativeViewportRuntime>>,
}

struct NativeViewportRuntime {
    window: Window,
    sender: Sender<RenderCommand>,
    thread: Option<JoinHandle<()>>,
}

impl NativeViewportRuntime {
    fn shutdown(mut self) {
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
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportVisibleRequest {
    visible: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportReport {
    stats: ViewportStats,
    camera: Option<CameraSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct NativeViewportPickReport {
    pick: Option<PickResult>,
    viewport: NativeViewportReport,
}

enum RenderCommand {
    Load {
        package: Box<RenderPackage>,
        reply: Sender<Result<NativeViewportReport, String>>,
    },
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
    Pick {
        x: f32,
        y: f32,
        reply: Sender<Result<NativeViewportPickReport, String>>,
    },
    Select {
        node_index: Option<u32>,
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
        reply: Sender<Result<NativeViewportReport, String>>,
    },
    Stats {
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

    let parent = app
        .get_window("main")
        .ok_or_else(|| "RageLab Studio main window is unavailable".to_string())?;

    let builder = tauri::window::WindowBuilder::new(&app, NATIVE_VIEWPORT_LABEL)
        .title("RageLab Native Viewport")
        .inner_size(f64::from(request.width), f64::from(request.height))
        .decorations(false)
        .shadow(false)
        .skip_taskbar(true)
        .resizable(false)
        .focused(false)
        .visible(false);
    let builder = builder.parent(&parent).map_err(core_error)?;
    let window = builder.build().map_err(core_error)?;
    window.set_ignore_cursor_events(true).map_err(core_error)?;

    let (sender, receiver) = mpsc::channel();
    let (ready_sender, ready_receiver) = mpsc::channel();
    let render_target = window.clone();
    let thread = thread::Builder::new()
        .name("ragelab-native-viewport".into())
        .spawn(move || render_thread(render_target, options, receiver, ready_sender))
        .map_err(core_error)?;

    let ready = ready_receiver
        .recv_timeout(RENDER_THREAD_READY_TIMEOUT)
        .map_err(|error| format!("Native viewport renderer did not initialize: {error}"))??;

    let resize_sender = sender.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(size) => {
            let _ = resize_sender.send(RenderCommand::Resize {
                width: size.width,
                height: size.height,
                reply: None,
            });
        }
        WindowEvent::ScaleFactorChanged { new_inner_size, .. } => {
            let _ = resize_sender.send(RenderCommand::Resize {
                width: new_inner_size.width,
                height: new_inner_size.height,
                reply: None,
            });
        }
        WindowEvent::Destroyed => {
            let _ = resize_sender.send(RenderCommand::Shutdown);
        }
        _ => {}
    });

    let mut runtime = state
        .runtime
        .lock()
        .map_err(|_| "Native viewport state lock is poisoned".to_string())?;
    *runtime = Some(NativeViewportRuntime {
        window,
        sender,
        thread: Some(thread),
    });

    Ok(ready)
}

#[tauri::command]
pub(crate) fn native_viewport_load_scene(
    state: tauri::State<'_, NativeViewportState>,
    request: WorkspaceSceneRequest,
) -> Result<NativeViewportReport, String> {
    let scene = validate_workspace_scene_context(
        &request.workspace,
        &request.ymap,
        &request.fallback_roots,
        &request.rpf_mounts,
        request.game_index.as_ref(),
        request.max_nodes,
    )?;
    let package = workspace_scene_render_package_with_game_index(
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
    .map_err(core_error)?;

    with_runtime(&state, |runtime| {
        request_report(&runtime.sender, |reply| RenderCommand::Load {
            package: Box::new(package),
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
    if !matches!(request.kind.as_str(), "orbit" | "pan" | "zoom" | "fly") {
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
) {
    let mut renderer = match SurfaceRenderer::new(target, options) {
        Ok(renderer) => renderer,
        Err(error) => {
            let _ = ready.send(Err(error.to_string()));
            return;
        }
    };

    let _ = ready.send(Ok(report(&renderer)));
    let mut has_scene = false;

    while let Ok(command) = receiver.recv() {
        match command {
            RenderCommand::Load { package, reply } => {
                let result = renderer
                    .set_package(*package)
                    .and_then(|_| renderer.render_frame())
                    .map(|_| {
                        has_scene = true;
                        report(&renderer)
                    })
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
            RenderCommand::Resize {
                width,
                height,
                reply,
            } => {
                let result = renderer
                    .resize(width, height)
                    .and_then(|_| render_if_loaded(&mut renderer, has_scene))
                    .map(|_| report(&renderer))
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
                send_after_render(&mut renderer, has_scene, reply);
            }
            RenderCommand::Pan {
                delta_x,
                delta_y,
                reply,
            } => {
                renderer.pan(delta_x, delta_y);
                send_after_render(&mut renderer, has_scene, reply);
            }
            RenderCommand::Zoom { delta, reply } => {
                renderer.zoom(delta);
                send_after_render(&mut renderer, has_scene, reply);
            }
            RenderCommand::Fly {
                forward,
                right,
                up,
                reply,
            } => {
                renderer.fly(forward, right, up);
                send_after_render(&mut renderer, has_scene, reply);
            }
            RenderCommand::Pick { x, y, reply } => {
                let pick = renderer.pick(x, y);
                renderer.select(pick.map(|value| value.node_index));
                let result = render_if_loaded(&mut renderer, has_scene)
                    .map(|_| NativeViewportPickReport {
                        pick,
                        viewport: report(&renderer),
                    })
                    .map_err(|error| error.to_string());
                let _ = reply.send(result);
            }
            RenderCommand::Select { node_index, reply } => {
                renderer.select(node_index);
                send_after_render(&mut renderer, has_scene, reply);
            }
            RenderCommand::Projection { projection, reply } => {
                renderer.set_projection(projection);
                send_after_render(&mut renderer, has_scene, reply);
            }
            RenderCommand::Overlays {
                grid,
                wireframe,
                bounds,
                reply,
            } => {
                renderer.set_overlays(grid, wireframe, bounds);
                send_after_render(&mut renderer, has_scene, reply);
            }
            RenderCommand::Stats { reply } => {
                let _ = reply.send(Ok(report(&renderer)));
            }
            RenderCommand::Shutdown => break,
        }
    }
}

fn send_after_render(
    renderer: &mut SurfaceRenderer,
    has_scene: bool,
    reply: Sender<Result<NativeViewportReport, String>>,
) {
    let result = render_if_loaded(renderer, has_scene)
        .map(|_| report(renderer))
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

fn report(renderer: &SurfaceRenderer) -> NativeViewportReport {
    NativeViewportReport {
        stats: renderer.stats(),
        camera: renderer.camera_snapshot(),
    }
}

#[cfg(debug_assertions)]
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeViewportSmokeSpec {
    output: String,
    scene: WorkspaceSceneRequest,
    #[serde(default = "smoke_width")]
    width: u32,
    #[serde(default = "smoke_height")]
    height: u32,
}

#[cfg(debug_assertions)]
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
    pick: Option<NativeViewportPickReport>,
    final_report: Option<NativeViewportReport>,
    error: Option<String>,
}

#[cfg(debug_assertions)]
const fn smoke_width() -> u32 {
    960
}

#[cfg(debug_assertions)]
const fn smoke_height() -> u32 {
    540
}

#[cfg(debug_assertions)]
pub(crate) fn maybe_start_debug_smoke(app: tauri::AppHandle, core_revision: &'static str) {
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

    tauri::async_runtime::spawn(async move {
        let result = run_debug_smoke(&app, &spec_path, core_revision).await;
        let exit_code = if result { 0 } else { 1 };
        app.exit(exit_code);
    });
}

#[cfg(debug_assertions)]
async fn run_debug_smoke(
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
        pick: None,
        final_report: None,
        error: None,
    };

    let smoke_result: Result<(), String> = async {
        let created = native_viewport_create(
            app.clone(),
            app.state::<NativeViewportState>(),
            NativeViewportCreateRequest {
                width: spec.width,
                height: spec.height,
            },
        )
        .await?;
        output.created = Some(created);

        native_viewport_set_visible(
            app.state::<NativeViewportState>(),
            NativeViewportVisibleRequest { visible: true },
        )?;

        let load_started = Instant::now();
        let loaded =
            native_viewport_load_scene(app.state::<NativeViewportState>(), spec.scene.clone())?;
        output.load_wall_ms = load_started.elapsed().as_secs_f64() * 1000.0;
        output.loaded = Some(loaded);

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
        native_viewport_shutdown(app.state::<NativeViewportState>())?;
        Ok(())
    }
    .await;

    match smoke_result {
        Ok(()) => output.ok = true,
        Err(error) => {
            output.error = Some(error);
            let _ = native_viewport_shutdown(app.state::<NativeViewportState>());
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
