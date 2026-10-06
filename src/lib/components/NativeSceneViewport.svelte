<script lang="ts">
  import { onMount } from "svelte";
  import type { UnlistenFn } from "@tauri-apps/api/event";
  import { getCurrentWindow } from "@tauri-apps/api/window";

  import {
    createNativeViewport,
    getNativeViewportStats,
    loadNativeViewportScene,
    pickNativeViewport,
    selectNativeViewportNode,
    sendNativeViewportInput,
    setNativeViewportOverlays,
    setNativeViewportProjection,
    setNativeViewportRect,
    setNativeViewportVisible,
    shutdownNativeViewport,
    type NativeViewportProjection,
    type NativeViewportReport,
    type SceneGameIndexSource,
    type SceneManifestReport,
    type SceneRpfMount,
  } from "$lib/native";

  export let manifest: SceneManifestReport | null = null;
  export let workspace: string;
  export let ymap: string;
  export let fallbackRoots: string[] = [];
  export let rpfMounts: SceneRpfMount[] = [];
  export let gameIndex: SceneGameIndexSource | null = null;
  export let maxNodes: number | undefined = undefined;
  export let selectedNodeIndex: number | null = null;
  export let onNativeFailure: ((message: string) => void) | null = null;

  let container: HTMLButtonElement;
  let mounted = false;
  let initialized = false;
  let loading = false;
  let renderError = "";
  let report: NativeViewportReport | null = null;
  let projection: NativeViewportProjection = "perspective";
  let grid = true;
  let wireframe = false;
  let bounds = false;
  let collision = false;
  let lastSceneKey = "";
  let previousSelectedNodeIndex: number | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let rectFrame = 0;
  let inputFrame = 0;
  let dragMode: "orbit" | "pan" | null = null;
  let pointerId: number | null = null;
  let pointerMoved = false;
  let pendingDeltaX = 0;
  let pendingDeltaY = 0;
  let unlistenWindowEvents: UnlistenFn[] = [];

  const currentWindow = getCurrentWindow();

  onMount(() => {
    mounted = true;
    void initializeNativeViewport();

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
      if (mounted) {
        unlistenWindowEvents = listeners;
      } else {
        for (const unlisten of listeners) {
          unlisten();
        }
      }
    });

    return () => {
      mounted = false;
      window.removeEventListener("scroll", schedule, true);
      window.removeEventListener("resize", schedule);
      resizeObserver?.disconnect();
      resizeObserver = null;
      for (const unlisten of unlistenWindowEvents) {
        unlisten();
      }
      unlistenWindowEvents = [];
      if (rectFrame) {
        cancelAnimationFrame(rectFrame);
      }
      if (inputFrame) {
        cancelAnimationFrame(inputFrame);
      }
      void setNativeViewportVisible(false).catch(() => {});
      void shutdownNativeViewport().catch(() => {});
    };
  });

  $: if (mounted && initialized && manifest) {
    const nextKey = sceneKey();
    if (nextKey !== lastSceneKey) {
      lastSceneKey = nextKey;
      void loadScene();
    }
  }

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

  async function initializeNativeViewport() {
    try {
      renderError = "";
      const width = Math.max(container.clientWidth, 1);
      const height = Math.max(container.clientHeight, 1);
      report = await createNativeViewport(width, height);
      initialized = true;
      await syncRect();
      await setNativeViewportOverlays(grid, wireframe, bounds, collision);
      await setNativeViewportVisible(true);
      if (manifest) {
        lastSceneKey = sceneKey();
        await loadScene();
      }
    } catch (error) {
      handleNativeError(error);
    }
  }

  async function loadScene() {
    if (!manifest || loading) {
      return;
    }

    loading = true;
    renderError = "";
    try {
      report = await loadNativeViewportScene({
        workspace,
        ymap,
        fallbackRoots,
        rpfMounts,
        gameIndex,
        maxNodes,
      });
      previousSelectedNodeIndex = selectedNodeIndex;
      if (selectedNodeIndex !== null) {
        report = await selectNativeViewportNode(selectedNodeIndex);
      }
      await setNativeViewportVisible(true);
    } catch (error) {
      handleNativeError(error);
    } finally {
      loading = false;
    }
  }

  function sceneKey() {
    return JSON.stringify({
      workspace,
      ymap,
      fallbackRoots,
      rpfMounts,
      gameIndex,
      maxNodes,
      root: manifest?.root.path ?? "",
      nodes: manifest?.summary.emittedNodes ?? 0,
      assets: manifest?.summary.assetReferences ?? 0,
    });
  }

  function scheduleRectSync() {
    if (!mounted || !initialized || rectFrame) {
      return;
    }
    rectFrame = requestAnimationFrame(() => {
      rectFrame = 0;
      void syncRect().catch(handleNativeError);
    });
  }

  async function syncRect() {
    if (!container || !initialized) {
      return;
    }
    const rect = container.getBoundingClientRect();
    const scale = await currentWindow.scaleFactor();
    const origin = await currentWindow.innerPosition();
    const width = Math.max(Math.round(rect.width * scale), 1);
    const height = Math.max(Math.round(rect.height * scale), 1);
    const x = Math.round(origin.x + rect.left * scale);
    const y = Math.round(origin.y + rect.top * scale);

    const viewportWidth = window.innerWidth * scale;
    const viewportHeight = window.innerHeight * scale;
    const visible =
      rect.bottom > 0 &&
      rect.right > 0 &&
      rect.top < window.innerHeight &&
      rect.left < window.innerWidth &&
      x + width > origin.x &&
      y + height > origin.y &&
      x < origin.x + viewportWidth &&
      y < origin.y + viewportHeight;

    if (!visible) {
      await setNativeViewportVisible(false);
      return;
    }

    report = await setNativeViewportRect({ x, y, width, height });
    await setNativeViewportVisible(true);
  }

  function pointerDown(event: PointerEvent) {
    if (!initialized || event.button > 2) {
      return;
    }
    container.focus();
    pointerId = event.pointerId;
    pointerMoved = false;
    dragMode = event.button === 0 ? "orbit" : "pan";
    container.setPointerCapture(event.pointerId);
    event.preventDefault();
  }

  function pointerMove(event: PointerEvent) {
    if (pointerId !== event.pointerId || !dragMode) {
      return;
    }
    const width = Math.max(container.clientWidth, 1);
    const height = Math.max(container.clientHeight, 1);
    pendingDeltaX += event.movementX / width;
    pendingDeltaY += event.movementY / height;
    pointerMoved ||= Math.abs(event.movementX) + Math.abs(event.movementY) > 1;
    scheduleInputFlush();
    event.preventDefault();
  }

  function pointerUp(event: PointerEvent) {
    if (pointerId !== event.pointerId) {
      return;
    }
    if (container.hasPointerCapture(event.pointerId)) {
      container.releasePointerCapture(event.pointerId);
    }
    const shouldPick = event.button === 0 && !pointerMoved;
    pointerId = null;
    dragMode = null;
    if (shouldPick) {
      void pickAt(event);
    }
    event.preventDefault();
  }

  function scheduleInputFlush() {
    if (inputFrame) {
      return;
    }
    inputFrame = requestAnimationFrame(() => {
      inputFrame = 0;
      const deltaX = pendingDeltaX;
      const deltaY = pendingDeltaY;
      pendingDeltaX = 0;
      pendingDeltaY = 0;
      const kind = dragMode;
      if (!kind || (deltaX === 0 && deltaY === 0)) {
        return;
      }
      void sendNativeViewportInput({ kind, deltaX, deltaY })
        .then((next) => {
          report = next;
        })
        .catch(handleNativeError);
    });
  }

  function wheel(event: WheelEvent) {
    if (!initialized) {
      return;
    }
    event.preventDefault();
    const delta = Math.max(-3, Math.min(3, event.deltaY / 240));
    void sendNativeViewportInput({ kind: "zoom", delta })
      .then((next) => {
        report = next;
      })
      .catch(handleNativeError);
  }

  function keyDown(event: KeyboardEvent) {
    if (!initialized || event.repeat) {
      return;
    }
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

    event.preventDefault();
    void sendNativeViewportInput({ kind: "fly", forward, right, up })
      .then((next) => {
        report = next;
      })
      .catch(handleNativeError);
  }

  async function pickAt(event: PointerEvent) {
    const rect = container.getBoundingClientRect();
    if (rect.width <= 0 || rect.height <= 0) {
      return;
    }
    const x = (event.clientX - rect.left) / rect.width;
    const y = (event.clientY - rect.top) / rect.height;
    try {
      const result = await pickNativeViewport(x, y);
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
      report = await setNativeViewportOverlays(grid, wireframe, bounds, collision);
    } catch (error) {
      handleNativeError(error);
    }
  }

  async function refreshStats() {
    try {
      report = await getNativeViewportStats();
    } catch (error) {
      handleNativeError(error);
    }
  }

  function handleNativeError(error: unknown) {
    const message =
      error instanceof Error ? error.message : String(error ?? "Native viewport failed.");
    renderError = message;
    void setNativeViewportVisible(false).catch(() => {});
    onNativeFailure?.(message);
  }

  function mib(bytes: number) {
    return (bytes / (1024 * 1024)).toFixed(1);
  }
</script>

<section class="scene-viewport">
  <div class="viewport-meta">
    <div>
      <p class="label">Native wgpu viewport</p>
      <div class="meta-line">
        <span>{report?.stats.instances ?? manifest?.summary.emittedNodes ?? 0} instances</span>
        <span>{report?.stats.gpuAssetCache ?? 0} GPU assets</span>
        <span>{report?.stats.gpuTextureCache ?? 0} GPU textures</span>
        <span>{mib(report?.stats.uploadedPayloadBytes ?? 0)} MiB payload</span>
        <span>{(report?.stats.sceneLoadMs ?? 0).toFixed(1)} ms load</span>
        <span>{(report?.stats.lastFrameMs ?? 0).toFixed(2)} ms frame</span>
        {#if selectedNodeIndex !== null}
          <span>selected #{selectedNodeIndex}</span>
        {/if}
        {#if loading}
          <span>loading…</span>
        {/if}
      </div>
    </div>

    <div class="viewport-actions">
      <button type="button" class:active={projection === "orthographic"} onclick={toggleProjection}>
        {projection === "perspective" ? "Perspective" : "Orthographic"}
      </button>
      <button type="button" class:active={grid} onclick={() => { grid = !grid; void updateOverlays(); }}>
        Grid
      </button>
      <button type="button" class:active={wireframe} onclick={() => { wireframe = !wireframe; void updateOverlays(); }}>
        Wire
      </button>
      <button type="button" class:active={bounds} onclick={() => { bounds = !bounds; void updateOverlays(); }}>
        Bounds
      </button>
      <button type="button" class:active={collision} onclick={() => { collision = !collision; void updateOverlays(); }}>
        Collision
      </button>
      <button type="button" onclick={refreshStats}>Stats</button>
    </div>
  </div>

  {#if renderError}
    <div class="render-error" role="alert">{renderError}</div>
  {/if}

  <button
    type="button"
    class="viewport native-input-surface"
    bind:this={container}
    aria-label="RageLab native scene viewport"
    onpointerdown={pointerDown}
    onpointermove={pointerMove}
    onpointerup={pointerUp}
    onpointercancel={pointerUp}
    oncontextmenu={(event) => event.preventDefault()}
    onwheel={wheel}
    onkeydown={keyDown}
  >
    {#if !initialized || loading}
      <span class="viewport-overlay">
        <span>{initialized ? "Loading native scene…" : "Starting native renderer…"}</span>
      </span>
    {/if}
  </button>

  <div class="viewport-hint">
    Left drag orbit · right/middle drag pan · wheel zoom · WASD/QE fly · click entity
  </div>
</section>

<style>
  .scene-viewport {
    border: 1px solid #292e35;
    border-radius: 10px;
    overflow: hidden;
    background: #0a0c0f;
  }

  .viewport-meta {
    min-height: 58px;
    padding: 10px 12px;
    border-bottom: 1px solid #292e35;
    background: #12151a;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .label {
    margin: 0 0 5px;
    color: #7d838c;
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .meta-line,
  .viewport-actions {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .meta-line span {
    padding: 3px 6px;
    border: 1px solid #2d3239;
    border-radius: 5px;
    color: #9299a3;
    font-size: 10px;
  }

  .viewport-actions {
    justify-content: flex-end;
  }

  .viewport-actions button {
    border: 1px solid #30363f;
    border-radius: 5px;
    background: #181c22;
    color: #858c96;
    padding: 5px 7px;
    font: inherit;
    font-size: 10px;
    cursor: pointer;
  }

  .viewport-actions button:hover,
  .viewport-actions button.active {
    border-color: #526170;
    color: #c0c8d2;
    background: #20262d;
  }

  .viewport {
    position: relative;
    width: 100%;
    height: min(58vh, 560px);
    min-height: 360px;
    padding: 0;
    border: 0;
    outline: none;
    background: #0a0c0f;
    color: inherit;
    font: inherit;
    text-align: left;
    cursor: crosshair;
  }

  .viewport:focus-visible {
    box-shadow: inset 0 0 0 1px #526170;
  }

  .viewport-overlay {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: #7d858f;
    font-size: 11px;
    pointer-events: none;
  }

  .viewport-hint {
    padding: 7px 12px;
    border-top: 1px solid #292e35;
    color: #626a74;
    background: #111419;
    font-size: 10px;
  }

  .render-error {
    padding: 10px 14px;
    border-bottom: 1px solid #493027;
    background: #1c1411;
    color: #d2a187;
    font-size: 11px;
  }

  @media (max-width: 700px) {
    .viewport-meta {
      display: grid;
    }

    .viewport-actions {
      justify-content: flex-start;
    }

    .viewport {
      height: 360px;
      min-height: 300px;
    }
  }
</style>
