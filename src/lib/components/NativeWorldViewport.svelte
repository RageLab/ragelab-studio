<script lang="ts">
  import { onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import {
    createNativeViewport,
    getNativeViewportStats,
    moveNativeWorldStream,
    pickNativeViewport,
    selectNativeViewportNode,
    sendNativeViewportInput,
    setNativeViewportOverlays,
    setNativeViewportProjection,
    setNativeViewportRect,
    setNativeViewportVisible,
    shutdownNativeViewport,
    startNativeWorldStream,
    type NativeViewportProjection,
    type NativeViewportReport,
    type SceneGameIndexSource,
  } from "$lib/native";

  export let gameIndex: SceneGameIndexSource;
  export let initialPosition: [number, number, number];
  export let selectedNodeIndex: number | null = null;
  export let report: NativeViewportReport | null = null;
  export let onNativeFailure: ((message: string) => void) | null = null;

  let container: HTMLButtonElement;
  let mounted = false;
  let initialized = false;
  let loading = false;
  let renderError = "";
  let projection: NativeViewportProjection = "perspective";
  let grid = true;
  let wireframe = false;
  let bounds = false;
  let previousSelectedNodeIndex: number | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let rectFrame = 0;
  let inputFrame = 0;
  let streamTimer: ReturnType<typeof setTimeout> | null = null;
  let streamGeneration = 0;
  let dragMode: "look" | "pan" | null = null;
  let pointerId: number | null = null;
  let pointerMoved = false;
  let pendingDeltaX = 0;
  let pendingDeltaY = 0;
  let unlistenWindowEvents: UnlistenFn[] = [];

  const currentWindow = getCurrentWindow();

  onMount(() => {
    mounted = true;
    void initialize();

    resizeObserver = new ResizeObserver(() => scheduleRectSync());
    resizeObserver.observe(container);

    const schedule = () => scheduleRectSync();
    window.addEventListener("scroll", schedule, true);
    window.addEventListener("resize", schedule);

    void Promise.all([
      currentWindow.onMoved(schedule),
      currentWindow.onResized(schedule),
      currentWindow.onScaleChanged(schedule),
      currentWindow.onFocusChanged(({ payload }) => {
        void setNativeViewportVisible(Boolean(payload) && initialized).catch(() => {});
      }),
    ]).then((listeners) => {
      if (mounted) unlistenWindowEvents = listeners;
      else listeners.forEach((unlisten) => unlisten());
    });

    return () => {
      mounted = false;
      window.removeEventListener("scroll", schedule, true);
      window.removeEventListener("resize", schedule);
      resizeObserver?.disconnect();
      resizeObserver = null;
      unlistenWindowEvents.forEach((unlisten) => unlisten());
      unlistenWindowEvents = [];
      if (rectFrame) cancelAnimationFrame(rectFrame);
      if (inputFrame) cancelAnimationFrame(inputFrame);
      if (streamTimer) clearTimeout(streamTimer);
      void setNativeViewportVisible(false).catch(() => {});
      void shutdownNativeViewport().catch(() => {});
    };
  });

  $: if (
    mounted &&
    initialized &&
    selectedNodeIndex !== previousSelectedNodeIndex
  ) {
    previousSelectedNodeIndex = selectedNodeIndex;
    void selectNativeViewportNode(selectedNodeIndex)
      .then((next) => {
        report = next;
      })
      .catch(handleNativeError);
  }

  async function initialize() {
    try {
      renderError = "";
      loading = true;
      const width = Math.max(container.clientWidth, 1);
      const height = Math.max(container.clientHeight, 1);
      report = await createNativeViewport(width, height);
      initialized = true;
      await syncRect();
      await setNativeViewportOverlays(grid, wireframe, bounds);
      report = await startNativeWorldStream({
        gameIndex,
        position: initialPosition,
        loadRadius: 300,
        retainRadius: 450,
        maxActiveMaps: 8,
        maxCpuChunks: 24,
      });
      await setNativeViewportVisible(true);
    } catch (error) {
      handleNativeError(error);
    } finally {
      loading = false;
    }
  }

  export async function teleport(
    position: [number, number, number],
    fitCamera = true,
  ) {
    if (!initialized) return;
    loading = true;
    try {
      report = await moveNativeWorldStream(position, fitCamera);
      selectedNodeIndex = null;
      previousSelectedNodeIndex = null;
    } catch (error) {
      handleNativeError(error);
    } finally {
      loading = false;
    }
  }

  export async function refresh() {
    if (!initialized) return;
    try {
      report = await getNativeViewportStats();
      selectedNodeIndex = report.stats.selectedNodeIndex;
      previousSelectedNodeIndex = selectedNodeIndex;
    } catch (error) {
      handleNativeError(error);
    }
  }

  function scheduleRectSync() {
    if (!mounted || !initialized || rectFrame) return;
    rectFrame = requestAnimationFrame(() => {
      rectFrame = 0;
      void syncRect().catch(handleNativeError);
    });
  }

  async function syncRect() {
    if (!container || !initialized) return;
    const rect = container.getBoundingClientRect();
    const scale = await currentWindow.scaleFactor();
    const origin = await currentWindow.innerPosition();
    const width = Math.max(Math.round(rect.width * scale), 1);
    const height = Math.max(Math.round(rect.height * scale), 1);
    const x = Math.round(origin.x + rect.left * scale);
    const y = Math.round(origin.y + rect.top * scale);
    const visible =
      rect.bottom > 0 &&
      rect.right > 0 &&
      rect.top < window.innerHeight &&
      rect.left < window.innerWidth;

    if (!visible) {
      await setNativeViewportVisible(false);
      return;
    }

    report = await setNativeViewportRect({ x, y, width, height });
    await setNativeViewportVisible(true);
  }

  function pointerDown(event: PointerEvent) {
    if (!initialized || event.button > 2) return;
    container.focus();
    pointerId = event.pointerId;
    pointerMoved = false;
    dragMode = event.button === 0 ? "look" : "pan";
    container.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function pointerMove(event: PointerEvent) {
    if (pointerId !== event.pointerId || !dragMode) return;
    pendingDeltaX += event.movementX / Math.max(container.clientWidth, 1);
    pendingDeltaY += event.movementY / Math.max(container.clientHeight, 1);
    pointerMoved ||= Math.abs(event.movementX) + Math.abs(event.movementY) > 1;
    scheduleInputFlush();
    event.preventDefault();
  }

  function pointerUp(event: PointerEvent) {
    if (pointerId !== event.pointerId) return;
    if (container.hasPointerCapture(event.pointerId)) {
      container.releasePointerCapture(event.pointerId);
    }
    const shouldPick = event.button === 0 && !pointerMoved;
    pointerId = null;
    dragMode = null;
    if (shouldPick) void pickAt(event);
    event.preventDefault();
  }

  function scheduleInputFlush() {
    if (inputFrame) return;
    inputFrame = requestAnimationFrame(() => {
      inputFrame = 0;
      const deltaX = pendingDeltaX;
      const deltaY = pendingDeltaY;
      pendingDeltaX = 0;
      pendingDeltaY = 0;
      const kind = dragMode;
      if (!kind || (deltaX === 0 && deltaY === 0)) return;
      void sendNativeViewportInput({ kind, deltaX, deltaY })
        .then(applyInputReport)
        .catch(handleNativeError);
    });
  }

  function wheel(event: WheelEvent) {
    if (!initialized) return;
    event.preventDefault();
    const delta = Math.max(-3, Math.min(3, event.deltaY / 240));
    void sendNativeViewportInput({ kind: "zoom", delta })
      .then(applyInputReport)
      .catch(handleNativeError);
  }

  function keyDown(event: KeyboardEvent) {
    if (!initialized || event.repeat) return;
    const key = event.key.toLowerCase();
    let forward = 0;
    let right = 0;
    let up = 0;
    if (key === "w" || key === "arrowup") forward = 1;
    else if (key === "s" || key === "arrowdown") forward = -1;
    else if (key === "d" || key === "arrowright") right = 1;
    else if (key === "a" || key === "arrowleft") right = -1;
    else if (key === "e") up = 1;
    else if (key === "q") up = -1;
    else return;

    const boost = event.shiftKey ? 3 : 1;
    event.preventDefault();
    void sendNativeViewportInput({
      kind: "fly",
      forward: forward * boost,
      right: right * boost,
      up: up * boost,
    })
      .then(applyInputReport)
      .catch(handleNativeError);
  }

  function applyInputReport(next: NativeViewportReport) {
    report = next;
    selectedNodeIndex = next.stats.selectedNodeIndex;
    previousSelectedNodeIndex = selectedNodeIndex;
    scheduleStreamSync(next);
  }

  function scheduleStreamSync(next: NativeViewportReport) {
    const eye = next.camera?.eye;
    if (!eye || !initialized) return;
    const generation = ++streamGeneration;
    if (streamTimer) clearTimeout(streamTimer);
    streamTimer = setTimeout(() => {
      if (!mounted || !initialized || generation !== streamGeneration) return;
      void moveNativeWorldStream(eye, false)
        .then((streamed) => {
          if (generation === streamGeneration) {
            report = streamed;
            selectedNodeIndex = streamed.stats.selectedNodeIndex;
            previousSelectedNodeIndex = selectedNodeIndex;
          }
        })
        .catch(handleNativeError);
    }, 180);
  }

  async function pickAt(event: PointerEvent) {
    const rect = container.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) return;
    try {
      const result = await pickNativeViewport(
        (event.clientX - rect.left) / rect.width,
        (event.clientY - rect.top) / rect.height,
      );
      report = result.viewport;
      selectedNodeIndex = result.pick?.nodeIndex ?? null;
      previousSelectedNodeIndex = selectedNodeIndex;
    } catch (error) {
      handleNativeError(error);
    }
  }

  async function toggleProjection() {
    projection = projection === "perspective" ? "orthographic" : "perspective";
    try {
      report = await setNativeViewportProjection(projection);
    } catch (error) {
      handleNativeError(error);
    }
  }

  async function updateOverlays() {
    try {
      report = await setNativeViewportOverlays(grid, wireframe, bounds);
    } catch (error) {
      handleNativeError(error);
    }
  }

  function handleNativeError(error: unknown) {
    const message =
      error instanceof Error ? error.message : String(error ?? "Native world viewport failed.");
    renderError = message;
    onNativeFailure?.(message);
  }

  function mib(bytes: number) {
    return (bytes / (1024 * 1024)).toFixed(1);
  }
</script>

<section class="world-viewport">
  <div class="viewport-meta">
    <div>
      <p class="label">Native GTA world</p>
      <div class="meta-line">
        <span>{report?.streaming?.activeMaps ?? 0} maps</span>
        <span>{report?.stats.instances ?? 0} instances</span>
        <span>{mib(report?.streaming?.cpuCachedBytes ?? 0)} MiB CPU</span>
        <span>{mib((report?.stats.gpuAssetCacheBytes ?? 0) + (report?.stats.gpuTextureCacheBytes ?? 0))} MiB GPU</span>
        <span>{report?.streaming?.cacheHits ?? 0} chunk hits</span>
        <span>{(report?.stats.lastFrameMs ?? 0).toFixed(2)} ms frame</span>
        {#if selectedNodeIndex !== null}<span>selected #{selectedNodeIndex}</span>{/if}
        {#if loading}<span>streaming…</span>{/if}
      </div>
    </div>
    <div class="viewport-actions">
      <button type="button" class:active={projection === "orthographic"} onclick={toggleProjection}>
        {projection === "perspective" ? "Perspective" : "Orthographic"}
      </button>
      <button type="button" class:active={grid} onclick={() => { grid = !grid; void updateOverlays(); }}>Grid</button>
      <button type="button" class:active={wireframe} onclick={() => { wireframe = !wireframe; void updateOverlays(); }}>Wire</button>
      <button type="button" class:active={bounds} onclick={() => { bounds = !bounds; void updateOverlays(); }}>Bounds</button>
      <button type="button" onclick={() => void refresh()}>Stats</button>
    </div>
  </div>

  {#if renderError}<div class="render-error" role="alert">{renderError}</div>{/if}

  <button
    type="button"
    class="viewport"
    bind:this={container}
    aria-label="RageLab streamed GTA world viewport"
    onpointerdown={pointerDown}
    onpointermove={pointerMove}
    onpointerup={pointerUp}
    onpointercancel={pointerUp}
    oncontextmenu={(event) => event.preventDefault()}
    onwheel={wheel}
    onkeydown={keyDown}
  >
    {#if !initialized || loading}
      <span class="viewport-overlay">{initialized ? "Streaming world…" : "Starting native renderer…"}</span>
    {/if}
  </button>

  <div class="viewport-hint">
    Left drag look · right/middle drag pan · wheel zoom · camera-relative WASD/QE · click entity
  </div>
</section>

<style>
  .world-viewport { border: 1px solid #292e35; border-radius: 10px; overflow: hidden; background: #0a0c0f; }
  .viewport-meta { min-height: 58px; padding: 10px 12px; border-bottom: 1px solid #292e35; background: #12151a; display: flex; align-items: center; justify-content: space-between; gap: 16px; }
  .label { margin: 0 0 5px; color: #7d838c; font-size: 11px; font-weight: 650; letter-spacing: .04em; text-transform: uppercase; }
  .meta-line,.viewport-actions { display: flex; flex-wrap: wrap; gap: 6px; }
  .meta-line span { padding: 3px 6px; border: 1px solid #2d3239; border-radius: 5px; color: #9299a3; font-size: 10px; }
  .viewport-actions { justify-content: flex-end; }
  .viewport-actions button { border: 1px solid #30363f; border-radius: 5px; background: #181c22; color: #858c96; padding: 5px 7px; font: inherit; font-size: 10px; cursor: pointer; }
  .viewport-actions button:hover,.viewport-actions button.active { border-color: #526170; color: #c0c8d2; background: #20262d; }
  .viewport { position: relative; width: 100%; height: min(58vh,560px); min-height: 400px; padding: 0; border: 0; outline: none; background: #0a0c0f; color: inherit; font: inherit; text-align: left; cursor: crosshair; }
  .viewport:focus-visible { box-shadow: inset 0 0 0 1px #526170; }
  .viewport-overlay { position: absolute; inset: 0; display: grid; place-items: center; color: #7d858f; font-size: 11px; pointer-events: none; }
  .viewport-hint { padding: 7px 12px; border-top: 1px solid #292e35; color: #626a74; background: #111419; font-size: 10px; }
  .render-error { padding: 10px 14px; border-bottom: 1px solid #493027; background: #1c1411; color: #d2a187; font-size: 11px; }
  @media (max-width: 700px) {
    .viewport-meta { display: grid; }
    .viewport-actions { justify-content: flex-start; }
    .viewport { height: 360px; min-height: 300px; }
  }
</style>
