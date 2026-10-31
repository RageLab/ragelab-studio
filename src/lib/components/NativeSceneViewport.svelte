<script lang="ts">
  import { onMount } from "svelte";

  import {
    createDedicatedNativeViewport,
    focusNativeViewportWindow,
    getNativeViewportStats,
    loadNativeViewportScene,
    selectNativeViewportNode,
    setNativeViewportOverlays,
    setNativeViewportProjection,
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

  let mounted = false;
  let initialized = false;
  let opening = false;
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
  let pollTimer: ReturnType<typeof setInterval> | null = null;

  onMount(() => {
    mounted = true;
    if (manifest) void openSceneWindow();

    pollTimer = setInterval(() => {
      if (mounted && initialized && !opening && !loading) {
        void refresh().catch(() => {});
      }
    }, 300);

    return () => {
      mounted = false;
      if (pollTimer) clearInterval(pollTimer);
      pollTimer = null;
      void shutdownNativeViewport().catch(() => {});
    };
  });

  $: if (mounted && manifest) {
    const nextKey = sceneKey();
    if (nextKey !== lastSceneKey && !opening && !loading) {
      lastSceneKey = nextKey;
      void openSceneWindow();
    }
  }

  $: if (
    mounted &&
    initialized &&
    selectedNodeIndex !== previousSelectedNodeIndex
  ) {
    previousSelectedNodeIndex = selectedNodeIndex;
    void selectNativeViewportNode(selectedNodeIndex)
      .then(applyReport)
      .catch(handleNativeError);
  }

  async function openSceneWindow() {
    if (!manifest || opening) return;
    opening = true;
    loading = true;
    renderError = "";

    try {
      report = await createDedicatedNativeViewport(1280, 800);
      initialized = true;
      await setNativeViewportOverlays(grid, wireframe, bounds, collision);
      applyReport(
        await loadNativeViewportScene({
          workspace,
          ymap,
          fallbackRoots,
          rpfMounts,
          gameIndex,
          maxNodes,
        }),
      );
      previousSelectedNodeIndex = selectedNodeIndex;
      if (selectedNodeIndex !== null) {
        applyReport(await selectNativeViewportNode(selectedNodeIndex));
      }
    } catch (error) {
      initialized = false;
      handleNativeError(error);
    } finally {
      opening = false;
      loading = false;
    }
  }

  async function focusMap() {
    if (!initialized) {
      await openSceneWindow();
      return;
    }

    try {
      await focusNativeViewportWindow();
    } catch {
      initialized = false;
      await openSceneWindow();
    }
  }

  async function refresh() {
    if (!initialized) return;
    try {
      applyReport(await getNativeViewportStats());
    } catch {
      initialized = false;
      report = null;
    }
  }

  async function toggleProjection() {
    projection = projection === "perspective" ? "orthographic" : "perspective";
    try {
      applyReport(await setNativeViewportProjection(projection));
    } catch (error) {
      handleNativeError(error);
    }
  }

  async function updateOverlays() {
    try {
      applyReport(await setNativeViewportOverlays(grid, wireframe, bounds, collision));
    } catch (error) {
      handleNativeError(error);
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

  function applyReport(next: NativeViewportReport) {
    report = next;
    const nativeSelection = next.stats.selectedNodeIndex;
    if (nativeSelection !== selectedNodeIndex) {
      previousSelectedNodeIndex = nativeSelection;
      selectedNodeIndex = nativeSelection;
    }
  }

  function handleNativeError(error: unknown) {
    const message =
      error instanceof Error ? error.message : String(error ?? "Dedicated scene window failed.");
    renderError = message;
    onNativeFailure?.(message);
  }

  function mib(bytes: number) {
    return (bytes / (1024 * 1024)).toFixed(1);
  }
</script>

<section class="map-controller">
  <div class="map-copy">
    <p class="label">Dedicated native scene</p>
    <h3>{initialized ? "Scene is open in its own window" : opening ? "Opening scene window…" : "Scene window is closed"}</h3>
    <p>
      The wgpu viewport runs in a separate native window instead of being positioned over
      the Studio HTML. The inspector and authoring controls stay here.
    </p>
  </div>

  <div class="map-actions">
    <button class="primary" type="button" onclick={() => void focusMap()} disabled={opening || loading}>
      {initialized ? "Bring scene to front" : "Open scene window"}
    </button>
    <button type="button" class:active={projection === "orthographic"} onclick={() => void toggleProjection()} disabled={!initialized}>
      {projection === "perspective" ? "Perspective" : "Orthographic"}
    </button>
    <button type="button" class:active={grid} onclick={() => { grid = !grid; void updateOverlays(); }} disabled={!initialized}>Grid</button>
    <button type="button" class:active={wireframe} onclick={() => { wireframe = !wireframe; void updateOverlays(); }} disabled={!initialized}>Wire</button>
    <button type="button" class:active={bounds} onclick={() => { bounds = !bounds; void updateOverlays(); }} disabled={!initialized}>Bounds</button>
    <button type="button" class:active={collision} onclick={() => { collision = !collision; void updateOverlays(); }} disabled={!initialized}>Collision</button>
  </div>

  {#if report}
    <div class="stats">
      <span>{report.stats.instances} instances</span>
      <span>{report.stats.gpuAssetCache} GPU assets</span>
      <span>{report.stats.gpuTextureCache} GPU textures</span>
      <span>{mib(report.stats.uploadedPayloadBytes)} MiB payload</span>
      <span>{report.stats.sceneLoadMs.toFixed(1)} ms load</span>
      <span>{report.stats.lastFrameMs.toFixed(2)} ms frame</span>
      {#if selectedNodeIndex !== null}<span>selected #{selectedNodeIndex}</span>{/if}
    </div>
  {/if}

  {#if renderError}
    <div class="render-error" role="alert">{renderError}</div>
  {/if}

  <p class="hint">
    Scene window: left drag look · right/middle drag pan · WASD/QE fly · Shift = faster · +/- zoom · Esc closes it.
  </p>
</section>

<style>
  .map-controller {
    min-height: 160px;
    padding: 16px;
    border: 1px solid #29313a;
    border-radius: 10px;
    background: #0f1216;
    display: grid;
    gap: 12px;
  }
  .label { margin: 0 0 5px; color: #7d838c; font-size: 10px; font-weight: 700; letter-spacing: .05em; text-transform: uppercase; }
  h3 { margin: 0; font-size: 17px; }
  .map-copy p:not(.label) { margin: 7px 0 0; max-width: 680px; color: #858d98; font-size: 11px; line-height: 1.5; }
  .map-actions,.stats { display:flex; flex-wrap:wrap; gap:6px; align-items:center; }
  button { border:1px solid #30363f; border-radius:6px; background:#171b20; color:#aeb5be; padding:7px 9px; font:inherit; font-size:10px; cursor:pointer; }
  button:hover:not(:disabled),button.active { border-color:#526170; background:#20262d; color:#d0d6dd; }
  button.primary { border-color:#e5e7eb; background:#f3f4f6; color:#111318; font-weight:700; }
  button:disabled { opacity:.45; cursor:default; }
  .stats span { padding:3px 6px; border:1px solid #2d3239; border-radius:5px; color:#9299a3; font-size:9px; }
  .hint { margin:0; color:#69717c; font-size:9px; }
  .render-error { padding:9px 10px; border:1px solid #493027; border-radius:6px; background:#1c1411; color:#d2a187; font-size:10px; }
</style>
