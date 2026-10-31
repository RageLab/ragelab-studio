<script lang="ts">
  import {
    chooseSceneFallbackDirectory,
    chooseSceneRpfArchive,
    chooseSceneRpfKeysDirectory,
    chooseWorkspaceYmap,
    focusNativeViewportWindow,
    openDedicatedNativeScene,
    openDedicatedNativeWorld,
    prepareGtaRpfIndex,
    prepareGtaRpfKeys,
    shutdownNativeViewport,
    type SceneGameIndexSource,
    type SceneRpfMount,
  } from "$lib/native";

  export let workspacePath: string | null = null;
  export let gtaLegacyRoot: string | null = null;

  let opening = false;
  let errorMessage = "";
  let statusMessage = "";
  let gameIndexSource: SceneGameIndexSource | null = null;
  let gameIndexRoot: string | null = null;
  let gameIndexLoading = false;

  let lastYmap: string | null = null;
  let mapKind: "scene" | "world" | null = null;
  let mapOpen = false;

  let fallbackRoot: string | null = null;
  let rpfArchive: string | null = null;
  let rpfKeys: string | null = null;
  let rpfNested = "";
  let maxNodes = "";

  $: if (gameIndexRoot !== gtaLegacyRoot) {
    gameIndexSource = null;
    gameIndexRoot = null;
  }

  $: rpfMounts =
    rpfArchive && rpfKeys
      ? [
          {
            archive: rpfArchive,
            nested: rpfNested
              .split(/\r?\n/)
              .map((value) => value.trim())
              .filter(Boolean),
            keys: rpfKeys,
          },
        ]
      : [];

  async function ensureGameIndex(force = false) {
    if (!gtaLegacyRoot) return null;
    if (!force && gameIndexSource && gameIndexRoot === gtaLegacyRoot) {
      return gameIndexSource;
    }

    gameIndexLoading = true;
    statusMessage = "Preparing GTA Legacy index…";
    try {
      const keyReport = await prepareGtaRpfKeys(gtaLegacyRoot);
      const indexReport = await prepareGtaRpfIndex(gtaLegacyRoot, keyReport.cache);
      gameIndexSource = {
        gameRoot: gtaLegacyRoot,
        index: indexReport.index,
        keys: keyReport.cache,
      };
      gameIndexRoot = gtaLegacyRoot;
      statusMessage = indexReport.cacheHit
        ? "GTA Legacy index ready."
        : `GTA Legacy index ready · ${indexReport.build?.indexedFiles ?? 0} files indexed.`;
      return gameIndexSource;
    } finally {
      gameIndexLoading = false;
    }
  }

  function parsedMaxNodes() {
    const raw = maxNodes.trim();
    if (!raw) return undefined;
    const value = Number.parseInt(raw, 10);
    if (!Number.isFinite(value) || value <= 0) {
      throw new Error("Node limit must be a positive integer.");
    }
    return value;
  }

  async function openYmap() {
    if (!workspacePath || opening) return;
    errorMessage = "";

    try {
      const ymap = await chooseWorkspaceYmap(workspacePath);
      if (!ymap) return;

      opening = true;
      statusMessage = "Opening YMAP in RageLab Map…";

      let gameIndex: SceneGameIndexSource | null = null;
      if (gtaLegacyRoot) {
        try {
          gameIndex = await ensureGameIndex();
        } catch (error) {
          // A local YMAP can still open without GTA provider resolution.
          statusMessage = "GTA index unavailable; opening the workspace YMAP with local sources only.";
          console.warn("Unable to prepare GTA index for scene", error);
        }
      }

      await openDedicatedNativeScene({
        workspace: workspacePath,
        ymap,
        fallbackRoots: fallbackRoot ? [fallbackRoot] : [],
        rpfMounts,
        gameIndex,
        maxNodes: parsedMaxNodes(),
      });

      lastYmap = ymap;
      mapKind = "scene";
      mapOpen = true;
      statusMessage = "YMAP is open in the dedicated RageLab Map window.";
    } catch (error) {
      mapOpen = false;
      setError(error, "Unable to open YMAP in the dedicated map window.");
    } finally {
      opening = false;
    }
  }

  async function openWorld() {
    if (!gtaLegacyRoot || opening) {
      if (!gtaLegacyRoot) {
        errorMessage = "A valid GTA V Legacy installation is required to open the GTA world.";
      }
      return;
    }

    errorMessage = "";
    opening = true;
    statusMessage = "Preparing GTA world…";

    try {
      const gameIndex = await ensureGameIndex();
      if (!gameIndex) throw new Error("GTA Legacy index is unavailable.");

      await openDedicatedNativeWorld({
        gameIndex,
        position: [-1625, -850, 10],
        loadRadius: 1150,
        retainRadius: 1500,
        maxActiveMaps: 20,
        maxChunkLoadsPerUpdate: 2,
        maxCpuChunks: 48,
      });

      mapKind = "world";
      mapOpen = true;
      statusMessage = "GTA world is open in the dedicated RageLab Map window.";
    } catch (error) {
      mapOpen = false;
      setError(error, "Unable to open the GTA world.");
    } finally {
      opening = false;
    }
  }

  async function focusMap() {
    errorMessage = "";
    try {
      await focusNativeViewportWindow();
      mapOpen = true;
    } catch {
      mapOpen = false;
      if (mapKind === "scene" && lastYmap && workspacePath) {
        opening = true;
        try {
          await openDedicatedNativeScene({
            workspace: workspacePath,
            ymap: lastYmap,
            fallbackRoots: fallbackRoot ? [fallbackRoot] : [],
            rpfMounts,
            gameIndex: gameIndexSource,
            maxNodes: parsedMaxNodes(),
          });
          mapOpen = true;
          statusMessage = "YMAP reopened in RageLab Map.";
        } catch (error) {
          setError(error, "Unable to reopen the YMAP window.");
        } finally {
          opening = false;
        }
      } else if (mapKind === "world") {
        await openWorld();
      }
    }
  }

  async function closeMap() {
    try {
      await shutdownNativeViewport();
    } catch {
      // The window may already have been closed manually.
    }
    mapOpen = false;
    statusMessage = "Map window closed.";
  }

  async function chooseFallback() {
    const selected = await chooseSceneFallbackDirectory();
    if (selected) fallbackRoot = selected;
  }

  async function chooseRpf() {
    const selected = await chooseSceneRpfArchive();
    if (selected) rpfArchive = selected;
  }

  async function chooseKeys() {
    const selected = await chooseSceneRpfKeysDirectory();
    if (selected) rpfKeys = selected;
  }

  function setError(error: unknown, fallback: string) {
    const message = error instanceof Error ? error.message : String(error ?? fallback);
    errorMessage = message || fallback;
    statusMessage = "";
  }
</script>

<section class="launcher" aria-live="polite">
  <header>
    <div>
      <p class="label">Map launcher</p>
      <h2>Maps open outside the Studio UI.</h2>
      <p class="detail">
        YMAP scenes and the GTA world now run only in the dedicated native
        <strong>RageLab Map</strong> window. This page is just the launcher and configuration surface.
      </p>
    </div>
    {#if mapKind}
      <span class:live={mapOpen} class="map-state">
        {mapOpen ? "Map window active" : "Map window closed"}
      </span>
    {/if}
  </header>

  <div class="launch-grid">
    <article>
      <span class="kicker">Workspace</span>
      <h3>Open YMAP</h3>
      <p>Choose a map from the current workspace and load it directly into the native map window.</p>
      <button class="primary" type="button" onclick={() => void openYmap()} disabled={!workspacePath || opening}>
        {opening ? "Opening…" : "Choose YMAP & open map"}
      </button>
      {#if lastYmap}<code>{lastYmap}</code>{/if}
    </article>

    <article>
      <span class="kicker">GTA V Legacy · read-only</span>
      <h3>Open GTA world</h3>
      <p>Streams nearby base-game YMAPs from the persisted GTA index into the native map window.</p>
      <button class="primary" type="button" onclick={() => void openWorld()} disabled={!gtaLegacyRoot || opening || gameIndexLoading}>
        {gameIndexLoading ? "Preparing GTA index…" : opening ? "Opening…" : "Open GTA world map"}
      </button>
      {#if gtaLegacyRoot}<code>{gtaLegacyRoot}</code>{:else}<small>GTA V Legacy was not detected.</small>{/if}
    </article>
  </div>

  {#if mapKind}
    <div class="map-controls">
      <div>
        <strong>{mapKind === "world" ? "GTA world" : "Workspace YMAP"}</strong>
        <span>{mapOpen ? "Dedicated renderer is running." : "The map window is not currently reachable."}</span>
      </div>
      <button type="button" onclick={() => void focusMap()} disabled={opening}>Bring map to front</button>
      <button type="button" onclick={() => void closeMap()}>Close map</button>
    </div>
  {/if}

  <details class="advanced">
    <summary>Advanced scene sources</summary>
    <p>Only needed for custom RPF mounts or loose asset fallbacks. GTA base-game data remains read-only.</p>
    <div class="advanced-grid">
      <label>
        <span>Node limit</span>
        <input type="number" min="1" bind:value={maxNodes} placeholder="Core default" />
      </label>
      <button type="button" onclick={() => void chooseRpf()}>{rpfArchive ? "Change RPF" : "Add RPF archive"}</button>
      <button type="button" onclick={() => void chooseKeys()}>{rpfKeys ? "Change key store" : "Select key store"}</button>
      <button type="button" onclick={() => void chooseFallback()}>{fallbackRoot ? "Change loose fallback" : "Add loose fallback"}</button>
    </div>
    {#if rpfArchive}
      <label class="nested">
        <span>Nested RPF chain · one path per line</span>
        <textarea rows="3" bind:value={rpfNested}></textarea>
      </label>
    {/if}
    <div class="source-list">
      {#if rpfArchive}<code>RPF: {rpfArchive}</code>{/if}
      {#if rpfKeys}<code>Keys: {rpfKeys}</code>{/if}
      {#if fallbackRoot}<code>Fallback: {fallbackRoot}</code>{/if}
    </div>
  </details>

  {#if statusMessage}<div class="status">{statusMessage}</div>{/if}
  {#if errorMessage}<div class="error" role="alert">{errorMessage}</div>{/if}
</section>

<style>
  .launcher { margin-top:18px; display:grid; gap:14px; }
  header { padding:18px; border:1px solid #292f37; border-radius:10px; background:#111419; display:flex; justify-content:space-between; gap:24px; align-items:flex-start; }
  .label,.kicker { margin:0 0 6px; color:#747c87; font-size:9px; font-weight:750; letter-spacing:.07em; text-transform:uppercase; }
  h2,h3 { margin:0; }
  h2 { font-size:20px; }
  h3 { font-size:16px; }
  .detail, article p, .advanced > p { color:#858d98; font-size:11px; line-height:1.55; }
  .detail { max-width:720px; margin:7px 0 0; }
  .map-state { padding:4px 8px; border:1px solid #3a414b; border-radius:99px; color:#7e8792; font-size:9px; white-space:nowrap; }
  .map-state.live { border-color:#31503f; color:#93bca1; }
  .launch-grid { display:grid; grid-template-columns:repeat(2,minmax(0,1fr)); gap:12px; }
  article { min-height:190px; padding:20px; border:1px solid #292f37; border-radius:10px; background:#111419; display:grid; align-content:start; gap:8px; }
  article p { margin:0 0 8px; }
  button,input,textarea { font:inherit; }
  button { width:max-content; border:1px solid #343b45; border-radius:6px; padding:8px 10px; background:#171b20; color:#c7cdd5; font-size:10px; cursor:pointer; }
  button:hover:not(:disabled) { border-color:#515c69; background:#20262d; }
  button.primary { border-color:#e5e7eb; background:#f3f4f6; color:#111318; font-weight:700; }
  button:disabled { opacity:.45; cursor:default; }
  code { color:#7f8792; font:9px "SFMono-Regular",Consolas,monospace; overflow-wrap:anywhere; }
  small { color:#777f89; font-size:10px; }
  .map-controls { padding:12px 14px; border:1px solid #2d343d; border-radius:8px; background:#0f1216; display:flex; gap:8px; align-items:center; }
  .map-controls > div { flex:1; display:grid; gap:3px; }
  .map-controls strong { font-size:11px; }
  .map-controls span { color:#747d88; font-size:9px; }
  .advanced { border:1px solid #272d35; border-radius:8px; background:#0f1216; }
  .advanced summary { padding:10px 12px; color:#8d95a0; font-size:10px; font-weight:700; cursor:pointer; }
  .advanced > p { margin:0; padding:0 12px 10px; }
  .advanced-grid { padding:0 12px 12px; display:flex; flex-wrap:wrap; gap:8px; align-items:end; }
  .advanced-grid label,.nested { display:grid; gap:4px; color:#727a85; font-size:9px; }
  input,textarea { border:1px solid #343b45; border-radius:5px; padding:7px 8px; background:#0b0e12; color:#d2d6dc; font-size:10px; }
  .nested { padding:0 12px 12px; }
  .nested textarea { width:100%; resize:vertical; }
  .source-list { padding:0 12px 12px; display:grid; gap:3px; }
  .status,.error { padding:10px 12px; border-radius:7px; font-size:10px; }
  .status { border:1px solid #2e3b33; background:#101611; color:#8eaa97; }
  .error { border:1px solid #54362d; background:#1b1210; color:#d2a187; }
  @media (max-width:800px) { .launch-grid { grid-template-columns:1fr; } header,.map-controls { display:grid; } }
</style>
