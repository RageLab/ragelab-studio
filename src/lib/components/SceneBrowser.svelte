<script lang="ts">
  import AuthoringPanel from "$lib/components/AuthoringPanel.svelte";
  import NativeSceneViewport from "$lib/components/NativeSceneViewport.svelte";
  import SceneViewport from "$lib/components/SceneViewport.svelte";
  import WorldBrowser from "$lib/components/WorldBrowser.svelte";
  import {
    assembleWorkspaceScene,
    completeDebugThreeViewportBenchmark,
    chooseSceneFallbackDirectory,
    chooseSceneRpfArchive,
    chooseSceneRpfKeysDirectory,
    chooseWorkspaceYmap,
    prepareGtaRpfIndex,
    prepareGtaRpfKeys,
    type AssetPreviewReport,
    type DebugThreeViewportBenchmarkSpec,
    type SceneGameIndexSource,
    type SceneManifestReport,
    type SceneNodeReport,
    type SceneRpfMount,
    type YmapAuthoringSnapshot,
  } from "$lib/native";
  import {
    loadSceneAssetPreviews,
    type ScenePreviewLoadResult,
  } from "$lib/scenePreviews";
  import {
    EMPTY_THREE_VIEWPORT_METRICS,
    type ThreeViewportMetrics,
  } from "$lib/viewportMetrics";

  export let workspacePath: string | null = null;
  export let gtaLegacyRoot: string | null = null;
  export let debugThreeBenchmark: DebugThreeViewportBenchmarkSpec | null = null;

  let activeWorkspacePath: string | null = workspacePath;
  let activeGtaLegacyRoot: string | null = gtaLegacyRoot;
  let ymapPath: string | null = null;
  let fallbackRoot: string | null = null;
  let rpfArchive: string | null = null;
  let rpfKeys: string | null = null;
  let rpfNested = "";
  let rpfMounts: SceneRpfMount[] = [];
  let manifest: SceneManifestReport | null = null;
  let loading = false;
  let errorMessage = "";
  let maxNodes = "";
  let selectedNodeIndex: number | null = null;
  let selectedNode: SceneNodeReport | null = null;
  let selectedPreviewError = "";
  let unresolvedNodes: SceneNodeReport[] = [];
  let collisionNodes: SceneNodeReport[] = [];
  let assetPreviews: Record<number, AssetPreviewReport> = {};
  let previewLoad: ScenePreviewLoadResult | null = null;
  let previewLoading = false;
  let keyPreparationLoading = false;
  let keyPreparationMessage = "";
  let gameIndexLoading = false;
  let gameIndexMessage = "";
  let gameIndexSource: SceneGameIndexSource | null = null;
  let gameIndexRoot: string | null = null;
  let sceneGeneration = 0;
  let viewportMode: "three" | "native" = "native";
  let nativeViewportError = "";
  let worldBrowserEnabled = false;
  let authoringPanel: {
    placeArchetype: (
      archetypeHash: number,
      position?: [number, number, number],
    ) => Promise<void>;
  } | null = null;
  let threeMetrics: ThreeViewportMetrics = {
    ...EMPTY_THREE_VIEWPORT_METRICS,
  };
  let sceneAssemblyMs = 0;
  let previewLoadMs = 0;
  let totalLoadMs = 0;
  let previewJsonBytes = 0;
  let debugBenchmarkStarted = false;
  let debugBenchmarkSubmitted = false;

  $: if (workspacePath !== activeWorkspacePath) {
    activeWorkspacePath = workspacePath;
    ymapPath = null;
    manifest = null;
    errorMessage = "";
    maxNodes = "";
    selectedNodeIndex = null;
    assetPreviews = {};
    previewLoad = null;
    previewLoading = false;
    sceneGeneration += 1;
  }

  $: if (gtaLegacyRoot !== activeGtaLegacyRoot) {
    activeGtaLegacyRoot = gtaLegacyRoot;
    gameIndexSource = null;
    gameIndexRoot = null;
    gameIndexMessage = "";
    worldBrowserEnabled = false;
  }

  $: rpfMounts =
    rpfArchive && rpfKeys
      ? [
          {
            archive: rpfArchive,
            keys: rpfKeys,
            nested: rpfNested
              .split(/\r?\n/)
              .map((value) => value.trim())
              .filter(Boolean),
          },
        ]
      : [];

  $: selectedNode =
    manifest && selectedNodeIndex !== null
      ? (manifest.nodes.find((node) => node.index === selectedNodeIndex) ?? null)
      : null;

  $: unresolvedNodes =
    manifest?.nodes.filter((node) => node.resolution === "unresolved") ?? [];

  $: collisionNodes =
    manifest?.nodes.filter((node) => node.collision !== null) ?? [];

  $: if (
    debugThreeBenchmark &&
    workspacePath &&
    activeWorkspacePath === workspacePath &&
    gtaLegacyRoot === debugThreeBenchmark.gtaLegacyRoot &&
    !debugBenchmarkStarted
  ) {
    debugBenchmarkStarted = true;
    viewportMode = "three";
    ymapPath = debugThreeBenchmark.ymap;
    maxNodes =
      debugThreeBenchmark.maxNodes === null
        ? ""
        : String(debugThreeBenchmark.maxNodes);
    void loadScene();
  }

  $: if (
    debugThreeBenchmark &&
    !debugBenchmarkSubmitted &&
    manifest &&
    !loading &&
    !previewLoading &&
    threeMetrics.realGeometryNodes > 0 &&
    threeMetrics.sharedGeometryAssets > 0 &&
    threeMetrics.sampledFrames >= 90
  ) {
    debugBenchmarkSubmitted = true;
    void submitThreeBenchmark();
  }

  $: selectedPreviewError =
    selectedNode?.assetRef !== null && selectedNode?.assetRef !== undefined
      ? (previewLoad?.errors[selectedNode.assetRef] ?? "")
      : "";

  async function chooseYmap() {
    if (!workspacePath) {
      errorMessage = "Open a workspace before choosing a YMAP.";
      return;
    }

    errorMessage = "";

    try {
      const selected = await chooseWorkspaceYmap(workspacePath);
      if (!selected) {
        return;
      }

      ymapPath = selected;
      manifest = null;
      selectedNodeIndex = null;
      assetPreviews = {};
      previewLoad = null;
      sceneGeneration += 1;
      await loadScene();
    } catch (error) {
      errorMessage = errorMessageFor(error, "Unable to choose a workspace YMAP.");
    }
  }

  async function chooseFallbackRoot() {
    errorMessage = "";

    try {
      const selected = await chooseSceneFallbackDirectory();
      if (!selected) {
        return;
      }

      fallbackRoot = selected;
      manifest = null;
      selectedNodeIndex = null;
      assetPreviews = {};
      previewLoad = null;
      sceneGeneration += 1;

      if (ymapPath) {
        await loadScene();
      }
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "Unable to choose an optional loose asset fallback.",
      );
    }
  }

  async function clearFallbackRoot() {
    fallbackRoot = null;
    manifest = null;
    selectedNodeIndex = null;
    assetPreviews = {};
    previewLoad = null;
    sceneGeneration += 1;

    if (ymapPath) {
      await loadScene();
    }
  }

  async function prepareKeysFromDetectedGta(reload = true) {
    if (!gtaLegacyRoot) {
      errorMessage =
        "Detect a valid GTA V Legacy installation before preparing RPF keys.";
      return;
    }

    errorMessage = "";
    keyPreparationMessage = "";
    keyPreparationLoading = true;

    try {
      const report = await prepareGtaRpfKeys(gtaLegacyRoot);
      rpfKeys = report.cache;
      keyPreparationMessage = report.cacheHit
        ? "Reused the cached GTA Legacy RPF keys."
        : "Prepared GTA Legacy RPF keys from GTA5.exe.";

      manifest = null;
      selectedNodeIndex = null;
      assetPreviews = {};
      previewLoad = null;
      sceneGeneration += 1;

      if (reload && ymapPath && rpfArchive) {
        await loadScene();
      }
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "Unable to prepare GTA Legacy RPF keys.",
      );
    } finally {
      keyPreparationLoading = false;
    }
  }

  async function ensureDetectedGameIndex(
    force = false,
  ): Promise<SceneGameIndexSource | null> {
    if (!gtaLegacyRoot) {
      return null;
    }
    if (!force && gameIndexSource && gameIndexRoot === gtaLegacyRoot) {
      return gameIndexSource;
    }

    gameIndexLoading = true;
    gameIndexMessage = "";

    try {
      const keyReport = await prepareGtaRpfKeys(gtaLegacyRoot);
      const indexReport = await prepareGtaRpfIndex(
        gtaLegacyRoot,
        keyReport.cache,
      );

      gameIndexSource = {
        gameRoot: gtaLegacyRoot,
        index: indexReport.index,
        keys: keyReport.cache,
      };
      gameIndexRoot = gtaLegacyRoot;
      gameIndexMessage = indexReport.cacheHit
        ? "Reused the cached GTA Legacy asset index."
        : "Indexed " + (indexReport.build?.indexedFiles ?? 0) + " GTA asset entries.";

      return gameIndexSource;
    } catch (error) {
      gameIndexSource = null;
      gameIndexRoot = null;
      throw error;
    } finally {
      gameIndexLoading = false;
    }
  }

  async function toggleWorldBrowser() {
    if (worldBrowserEnabled) {
      worldBrowserEnabled = false;
      return;
    }
    if (!gtaLegacyRoot) {
      errorMessage = "Detect a valid GTA V Legacy installation before opening the world browser.";
      return;
    }

    errorMessage = "";
    try {
      const source = await ensureDetectedGameIndex();
      if (source) worldBrowserEnabled = true;
    } catch (error) {
      errorMessage = errorMessageFor(error, "Unable to prepare the GTA world browser.");
    }
  }

  async function refreshDetectedGameIndex() {
    if (!gtaLegacyRoot) {
      errorMessage =
        "Detect a valid GTA V Legacy installation before preparing the game index.";
      return;
    }

    errorMessage = "";
    try {
      await ensureDetectedGameIndex(true);
      manifest = null;
      selectedNodeIndex = null;
      assetPreviews = {};
      previewLoad = null;
      sceneGeneration += 1;
      if (ymapPath) {
        await loadScene();
      }
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "Unable to prepare the GTA Legacy asset index.",
      );
    }
  }

  async function chooseRpfArchive() {
    errorMessage = "";

    try {
      const selected = await chooseSceneRpfArchive();
      if (!selected) {
        return;
      }

      rpfArchive = selected;
      if (!rpfKeys && gtaLegacyRoot) {
        await prepareKeysFromDetectedGta(false);
      }
      manifest = null;
      selectedNodeIndex = null;
      assetPreviews = {};
      previewLoad = null;
      sceneGeneration += 1;

      if (ymapPath && rpfKeys) {
        await loadScene();
      }
    } catch (error) {
      errorMessage = errorMessageFor(error, "Unable to choose an RPF archive.");
    }
  }

  async function chooseRpfKeys() {
    errorMessage = "";

    try {
      const selected = await chooseSceneRpfKeysDirectory();
      if (!selected) {
        return;
      }

      rpfKeys = selected;
      keyPreparationMessage = "Using the manually selected RPF key store.";
      manifest = null;
      selectedNodeIndex = null;
      assetPreviews = {};
      previewLoad = null;
      sceneGeneration += 1;

      if (ymapPath && rpfArchive) {
        await loadScene();
      }
    } catch (error) {
      errorMessage = errorMessageFor(error, "Unable to choose an RPF key store.");
    }
  }

  async function clearRpfSource() {
    rpfArchive = null;
    rpfKeys = null;
    keyPreparationMessage = "";
    rpfNested = "";
    manifest = null;
    selectedNodeIndex = null;
    assetPreviews = {};
    previewLoad = null;
    sceneGeneration += 1;

    if (ymapPath) {
      await loadScene();
    }
  }

  async function reloadScene() {
    if (ymapPath) {
      await loadScene();
    }
  }

  async function loadScene() {
    if (!workspacePath || !ymapPath) {
      return;
    }

    const loadStarted = performance.now();
    sceneAssemblyMs = 0;
    previewLoadMs = 0;
    totalLoadMs = 0;
    previewJsonBytes = 0;
    threeMetrics = { ...EMPTY_THREE_VIEWPORT_METRICS };

    const generation = ++sceneGeneration;
    errorMessage = "";
    loading = true;
    previewLoading = false;
    manifest = null;
    assetPreviews = {};
    previewLoad = null;
    selectedNodeIndex = null;

    let parsedMaxNodes: number | undefined;
    const rawMaxNodes = maxNodes.trim();
    if (rawMaxNodes) {
      parsedMaxNodes = Number(rawMaxNodes);
      if (!Number.isInteger(parsedMaxNodes) || parsedMaxNodes <= 0) {
        errorMessage = "maxNodes must be a positive integer.";
        loading = false;
        return;
      }
    }

    try {
      const fallbackRoots = fallbackRoot ? [fallbackRoot] : [];
      const mounts = rpfMounts;
      const gameIndex = await ensureDetectedGameIndex();

      const assemblyStarted = performance.now();
      const assembled = await assembleWorkspaceScene(
        workspacePath,
        ymapPath,
        parsedMaxNodes,
        fallbackRoots,
        mounts,
        gameIndex,
      );
      sceneAssemblyMs = performance.now() - assemblyStarted;

      if (generation !== sceneGeneration) {
        return;
      }

      manifest = assembled;
      loading = false;
      totalLoadMs = performance.now() - loadStarted;

      if (viewportMode === "three") {
        await loadThreePreviews(
          assembled,
          generation,
          parsedMaxNodes,
          fallbackRoots,
          mounts,
          gameIndex,
          loadStarted,
        );
      }
    } catch (error) {
      if (generation === sceneGeneration) {
        errorMessage = errorMessageFor(
          error,
          "RageLab core could not assemble or preview this workspace scene.",
        );
      }
    } finally {
      if (generation === sceneGeneration) {
        loading = false;
        previewLoading = false;
      }
    }
  }

  async function loadThreePreviews(
    assembled: SceneManifestReport,
    generation: number,
    parsedMaxNodes: number | undefined,
    fallbackRoots: string[],
    mounts: SceneRpfMount[],
    gameIndex: SceneGameIndexSource | null,
    loadStarted?: number,
  ) {
    if (
      generation !== sceneGeneration ||
      previewLoading ||
      previewLoad !== null ||
      Object.keys(assetPreviews).length > 0
    ) {
      return;
    }

    previewLoading = true;
    try {
      const previewStarted = performance.now();
      const loaded = await loadSceneAssetPreviews(assembled, {
        workspace: workspacePath!,
        ymap: ymapPath!,
        maxNodes: parsedMaxNodes,
        fallbackRoots,
        rpfMounts: mounts,
        gameIndex,
      });
      previewLoadMs = performance.now() - previewStarted;
      previewJsonBytes = new TextEncoder().encode(
        JSON.stringify(loaded.previews),
      ).byteLength;
      totalLoadMs =
        loadStarted === undefined
          ? sceneAssemblyMs + previewLoadMs
          : performance.now() - loadStarted;

      if (generation !== sceneGeneration) {
        return;
      }

      assetPreviews = loaded.previews;
      previewLoad = loaded;
    } finally {
      if (generation === sceneGeneration) {
        previewLoading = false;
      }
    }
  }

  async function activateViewport(mode: "three" | "native") {
    viewportMode = mode;
    if (mode === "native") {
      nativeViewportError = "";
      return;
    }

    if (!manifest || !workspacePath || !ymapPath) {
      return;
    }

    try {
      let parsedMaxNodes: number | undefined;
      const rawMaxNodes = maxNodes.trim();
      if (rawMaxNodes) {
        parsedMaxNodes = Number(rawMaxNodes);
      }
      const fallbackRoots = fallbackRoot ? [fallbackRoot] : [];
      const mounts = rpfMounts;
      const gameIndex = await ensureDetectedGameIndex();
      await loadThreePreviews(
        manifest,
        sceneGeneration,
        parsedMaxNodes,
        fallbackRoots,
        mounts,
        gameIndex,
      );
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "Unable to load the Three.js fallback previews.",
      );
    }
  }

  function handleNativeViewportFailure(message: string) {
    nativeViewportError = message;
    void activateViewport("three");
  }

  async function submitThreeBenchmark() {
    if (!debugThreeBenchmark || !manifest) {
      return;
    }

    try {
      await completeDebugThreeViewportBenchmark({
        ok: true,
        scene: {
          ymap: manifest.root.path,
          totalEntities: manifest.summary.totalEntities,
          emittedNodes: manifest.summary.emittedNodes,
          resolvedNodes: manifest.summary.resolvedNodes,
          unresolvedNodes: manifest.summary.unresolvedNodes,
          assetReferences: manifest.summary.assetReferences,
        },
        timings: {
          sceneAssemblyMs,
          previewLoadMs,
          totalLoadMs,
          sceneBuildMs: threeMetrics.sceneBuildMs,
          averageFrameMs: threeMetrics.averageFrameMs,
          sampledFrames: threeMetrics.sampledFrames,
        },
        transport: {
          previewJsonBytes,
          typedPayloadBytes: threeMetrics.payloadBytes,
        },
        webgl: {
          geometries: threeMetrics.rendererGeometries,
          textures: threeMetrics.rendererTextures,
          sharedGeometryAssets: threeMetrics.sharedGeometryAssets,
          sharedDiffuseTextures: threeMetrics.sharedDiffuseTextures,
          texturedMaterials: threeMetrics.texturedMaterials,
          realGeometryNodes: threeMetrics.realGeometryNodes,
          fallbackProxyNodes: threeMetrics.fallbackProxyNodes,
        },
        previews: {
          loaded: Object.keys(assetPreviews).length,
          eligibleAssets: previewLoad?.eligibleAssets ?? 0,
          requestedAssets: previewLoad?.requestedAssets ?? 0,
          reusedNodeReferences: previewLoad?.reusedNodeReferences ?? 0,
          omittedAssets: previewLoad?.omittedAssets ?? 0,
          skippedWithoutScale: previewLoad?.skippedWithoutScale ?? 0,
          errors: previewLoad ? Object.keys(previewLoad.errors).length : 0,
        },
      });
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "Unable to submit the automated Three.js viewport benchmark.",
      );
    }
  }

  function selectNode(node: SceneNodeReport) {
    selectedNodeIndex = node.index;
  }

  function handleAuthoringSnapshot(snapshot: YmapAuthoringSnapshot) {
    manifest = snapshot.manifest;
    if (
      selectedNodeIndex !== null &&
      !snapshot.manifest.nodes.some((node) => node.index === selectedNodeIndex)
    ) {
      selectedNodeIndex = null;
    }
  }

  async function placeArchetypeFromWorld(
    archetypeHash: number,
    position: [number, number, number] | null,
  ) {
    if (!authoringPanel) {
      throw new Error("Open a workspace YMAP scene before placing an archetype.");
    }
    await authoringPanel.placeArchetype(archetypeHash, position ?? undefined);
  }

  function errorMessageFor(error: unknown, fallback: string): string {
    return error instanceof Error && error.message ? error.message : fallback;
  }
</script>

<section class="scene-browser" id="scene-browser" aria-live="polite">
  <div class="heading">
    <div>
      <p class="label">Workspace scene</p>
      <h2>YMAP scene manifest</h2>
      <p class="detail">
        Provider resolution, asset references, transforms, collision state, limits,
        and unresolved reasons come directly from RageLab core. A detected GTA V Legacy
        installation uses the native cached game index automatically; manual RPF mounts
        remain available only as an advanced override.
      </p>
    </div>

    <div class="actions">
      {#if gtaLegacyRoot}
        <button type="button" class:active={worldBrowserEnabled} onclick={() => void toggleWorldBrowser()} disabled={gameIndexLoading}>
          {worldBrowserEnabled ? "Close world browser" : gameIndexLoading ? "Preparing world…" : "World browser"}
        </button>
      {/if}

      <label class="max-nodes">
        <span>maxNodes override</span>
        <input
          type="number"
          min="1"
          step="1"
          placeholder="core default"
          bind:value={maxNodes}
          disabled={!workspacePath || loading}
        />
      </label>

      <button type="button" onclick={chooseRpfArchive} disabled={loading}>
        {rpfArchive ? "Change RPF" : "Add RPF source"}
      </button>

      {#if gtaLegacyRoot}
        <button
          type="button"
          onclick={refreshDetectedGameIndex}
          disabled={loading || gameIndexLoading}
        >
          {gameIndexLoading
            ? "Indexing GTA..."
            : gameIndexSource
              ? "Refresh GTA index"
              : "Prepare GTA index"}
        </button>
      {/if}

      <button
        type="button"
        onclick={chooseRpfKeys}
        disabled={loading || keyPreparationLoading}
      >
        {rpfKeys ? "Change RPF keys" : "Select RPF keys manually"}
      </button>

      <button type="button" onclick={chooseFallbackRoot} disabled={loading}>
        {fallbackRoot ? "Change loose fallback" : "Loose fallback"}
      </button>

      {#if fallbackRoot}
        <button type="button" onclick={clearFallbackRoot} disabled={loading}>
          Clear loose fallback
        </button>
      {/if}

      <button
        type="button"
        onclick={chooseYmap}
        disabled={!workspacePath || loading}
      >
        {gameIndexLoading
          ? "Indexing GTA..."
          : loading
            ? "Assembling..."
            : "Open workspace YMAP"}
      </button>
    </div>
  </div>

  {#if worldBrowserEnabled && gameIndexSource}
    <WorldBrowser
      gameIndex={gameIndexSource}
      {workspacePath}
      workspaceYmap={ymapPath}
      onPlaceArchetype={placeArchetypeFromWorld}
    />
  {/if}

  {#if rpfArchive || rpfKeys}
    <div class="source-config">
      <div class="source-fields">
        <div>
          <span>RPF archive</span>
          <code>{rpfArchive ?? "not selected"}</code>
        </div>
        <div>
          <span>RPF key store</span>
          <code>{rpfKeys ?? "not selected"}</code>
        </div>
      </div>

      <label class="nested-chain">
        <span>Nested RPF chain · one entry path per line</span>
        <textarea
          rows="3"
          bind:value={rpfNested}
          placeholder="x64/levels/gta5/.../metadata.rpf"
          disabled={loading}
        ></textarea>
      </label>

      <div class="source-actions">
        <button
          type="button"
          onclick={reloadScene}
          disabled={!ymapPath || !rpfArchive || !rpfKeys || loading}
        >
          Reload RPF source
        </button>
        <button type="button" onclick={clearRpfSource} disabled={loading}>
          Clear RPF source
        </button>
      </div>

      {#if keyPreparationMessage}
        <p class="source-note">{keyPreparationMessage}</p>
      {/if}

      {#if !gtaLegacyRoot && !rpfKeys}
        <p class="source-note">
          Detect GTA V Legacy to prepare the key cache automatically, or select an
          existing key store manually.
        </p>
      {:else if (rpfArchive && !rpfKeys) || (!rpfArchive && rpfKeys)}
        <p class="source-note">
          Both the RPF archive and key store are required before this source is mounted.
        </p>
      {/if}
    </div>
  {/if}

  {#if !workspacePath}
    <div class="empty">
      <strong>No workspace selected</strong>
      <p>Open a workspace first. Scene assembly is scoped to that root.</p>
    </div>
  {:else}
    <div class="workspace-path">
      <span>Workspace</span>
      <code>{workspacePath}</code>
      {#if gtaLegacyRoot}
        <span>GTA game index</span>
        <code>
          {gameIndexLoading
            ? "preparing..."
            : gameIndexSource?.index ?? "prepared on first scene load"}
        </code>
      {/if}
      {#if fallbackRoot}
        <span>Loose fallback</span>
        <code>{fallbackRoot}</code>
      {/if}
      {#if rpfArchive && rpfKeys}
        <span>RPF source</span>
        <code>
          {rpfArchive}{rpfNested.trim() ? " → " + rpfNested.trim().replace(/\r?\n/g, " → ") : ""}
        </code>
      {/if}
      {#if ymapPath}
        <span>YMAP</span>
        <code>{ymapPath}</code>
      {/if}
      {#if gameIndexMessage}
        <span>Index status</span>
        <code>{gameIndexMessage}</code>
      {/if}
    </div>

    {#if errorMessage}
      <div class="error" role="alert">{errorMessage}</div>
    {/if}

    {#if manifest}
      <div class="summary-grid">
        <article>
          <span>Total entities</span>
          <strong>{manifest.summary.totalEntities}</strong>
        </article>
        <article>
          <span>Emitted nodes</span>
          <strong>{manifest.summary.emittedNodes}</strong>
        </article>
        <article>
          <span>Resolved</span>
          <strong>{manifest.summary.resolvedNodes}</strong>
        </article>
        <article>
          <span>Unresolved</span>
          <strong>{manifest.summary.unresolvedNodes}</strong>
        </article>
        <article>
          <span>Asset refs</span>
          <strong>{manifest.summary.assetReferences}</strong>
        </article>
        <article>
          <span>Collision relations</span>
          <strong>{collisionNodes.length}</strong>
        </article>
      </div>

      <div class:truncated={manifest.limits.truncated} class="limits">
        <div>
          <span>Core node limit</span>
          <strong>{manifest.limits.maxNodes}</strong>
        </div>
        <div>
          <span>Truncated</span>
          <strong>{manifest.limits.truncated ? "Yes" : "No"}</strong>
        </div>
        <div>
          <span>Omitted entities</span>
          <strong>{manifest.limits.omittedEntities}</strong>
        </div>
      </div>

      {#if manifest.warnings.length > 0}
        <div class="warnings">
          <strong>Core warnings</strong>
          {#each manifest.warnings as warning}
            <p>{warning}</p>
          {/each}
        </div>
      {/if}

      {#if viewportMode === "three"}
        <div class:loading={previewLoading} class="preview-status">
          <div>
            <span>Resolved geometry</span>
            <strong>
              {Object.keys(assetPreviews).length}
              {previewLoading ? " loading…" : " asset preview(s)"}
            </strong>
          </div>
          <div>
            <span>UI preview budget</span>
            <strong>{previewLoad?.eligibleAssets ?? "—"} eligible</strong>
          </div>
          <div>
            <span>Reused node refs</span>
            <strong>{previewLoad?.reusedNodeReferences ?? 0}</strong>
          </div>
          <div>
            <span>Omitted by UI budget</span>
            <strong>{previewLoad?.omittedAssets ?? 0}</strong>
          </div>
          <div>
            <span>No explicit scale</span>
            <strong>{previewLoad?.skippedWithoutScale ?? 0}</strong>
          </div>
        </div>
      {/if}

      <div class="viewport-switcher" aria-label="Viewport renderer">
        <div>
          <span>Viewport renderer</span>
          <strong>{viewportMode === "native" ? "Native wgpu" : "Three.js fallback"}</strong>
        </div>
        <div class="viewport-switcher-actions">
          <button
            type="button"
            class:active={viewportMode === "native"}
            onclick={() => void activateViewport("native")}
          >
            Native wgpu
          </button>
          <button
            type="button"
            class:active={viewportMode === "three"}
            onclick={() => void activateViewport("three")}
          >
            Three.js
          </button>
        </div>
      </div>

      {#if nativeViewportError}
        <div class="native-viewport-warning">
          <strong>Native viewport fallback</strong>
          <span>{nativeViewportError}</span>
        </div>
      {/if}

      <div class="scene-layout">
        {#if
          viewportMode === "native" &&
          !worldBrowserEnabled &&
          activeWorkspacePath &&
          ymapPath
        }
          <NativeSceneViewport
            {manifest}
            workspace={activeWorkspacePath}
            ymap={ymapPath}
            fallbackRoots={fallbackRoot ? [fallbackRoot] : []}
            {rpfMounts}
            gameIndex={gameIndexSource}
            maxNodes={maxNodes.trim() ? Number.parseInt(maxNodes, 10) : undefined}
            bind:selectedNodeIndex
            onNativeFailure={handleNativeViewportFailure}
          />
        {:else}
          <SceneViewport
            {manifest}
            previews={assetPreviews}
            bind:selectedNodeIndex
            bind:metrics={threeMetrics}
          />
        {/if}

        <aside class="node-panel">
          <div class="node-panel-heading">
            <div>
              <p class="label">Node diagnostics</p>
              <h3>
                {selectedNode
                  ? "Node #" + selectedNode.index
                  : "Select a node"}
              </h3>
            </div>
            <span>{unresolvedNodes.length} unresolved</span>
          </div>

          {#if
            viewportMode === "native" &&
            activeWorkspacePath &&
            ymapPath &&
            manifest
          }
            <AuthoringPanel
              bind:this={authoringPanel}
              scene={{
                workspace: activeWorkspacePath,
                ymap: ymapPath,
                fallbackRoots: fallbackRoot ? [fallbackRoot] : [],
                rpfMounts,
                gameIndex: gameIndexSource,
                maxNodes: maxNodes.trim() ? Number.parseInt(maxNodes, 10) : undefined,
              }}
              {manifest}
              worldBrowserActive={worldBrowserEnabled}
              bind:selectedNodeIndex
              onSnapshot={handleAuthoringSnapshot}
            />
          {/if}

          {#if selectedNode}
            <div class="node-details">
              <div>
                <span>Resolution</span>
                <strong data-state={selectedNode.resolution}>
                  {selectedNode.resolution}
                </strong>
              </div>
              <div>
                <span>Archetype</span>
                <code>{selectedNode.archetypeHash}</code>
              </div>
              <div>
                <span>Asset kind</span>
                <strong>{selectedNode.assetKind ?? "—"}</strong>
              </div>
              <div>
                <span>Asset ref</span>
                <strong>{selectedNode.assetRef ?? "—"}</strong>
              </div>
              <div>
                <span>Provider</span>
                <code>{selectedNode.providerPath ?? "—"}</code>
              </div>
              <div>
                <span>Entity index</span>
                <strong>{selectedNode.entityIndex}</strong>
              </div>
            </div>

            {#if selectedNode.transform}
              <div class="transform">
                <span>Core world transform</span>
                <code>T [{selectedNode.transform.translation.join(", ")}]</code>
                <code>Q [{selectedNode.transform.rotation.join(", ")}]</code>
                <code>
                  S [{selectedNode.transform.scale?.join(", ") ?? "not provided"}]
                </code>
              </div>
            {:else}
              <div class="diagnostic">
                No usable world transform was returned for this node.
              </div>
            {/if}

            {#if selectedPreviewError}
              <div class="diagnostic preview-failure">
                <strong>Asset preview unavailable</strong>
                <span>{selectedPreviewError}</span>
              </div>
            {/if}

            {#if selectedNode.reason}
              <div class="diagnostic unresolved">
                <strong>{selectedNode.reason.code}</strong>
                <span>{selectedNode.reason.message}</span>
              </div>
            {/if}

            {#if selectedNode.collision}
              <div class="collision">
                <div>
                  <span>Collision</span>
                  <strong data-state={selectedNode.collision.state}>
                    {selectedNode.collision.state}
                  </strong>
                </div>
                <code>{selectedNode.collision.hash}</code>
                <span>{selectedNode.collision.reason}</span>
              </div>
            {/if}
          {:else}
            <p class="node-placeholder">
              Click a proxy in the viewport or choose an unresolved node below.
            </p>
          {/if}

          {#if unresolvedNodes.length > 0}
            <div class="unresolved-list">
              <span class="section-title">Unresolved nodes</span>
              {#each unresolvedNodes.slice(0, 100) as node}
                <button
                  class:selected={node.index === selectedNodeIndex}
                  type="button"
                  onclick={() => selectNode(node)}
                >
                  <span>#{node.index} · {node.archetypeHash}</span>
                  <small>{node.reason?.code ?? "unresolved"}</small>
                </button>
              {/each}
              {#if unresolvedNodes.length > 100}
                <p>
                  Showing 100 of {unresolvedNodes.length} unresolved nodes.
                </p>
              {/if}
            </div>
          {/if}
        </aside>
      </div>
    {:else if !errorMessage}
      <div class="empty scene-empty">
        <strong>No scene assembled</strong>
        <p>
          Choose a .ymap inside the selected workspace. RageLab core will scan
          providers and return the bounded scene manifest.
        </p>
      </div>
    {/if}
  {/if}
</section>

<style>
  .scene-browser {
    margin-top: 18px;
    padding: 24px;
    border: 1px solid #25292f;
    border-radius: 12px;
    background: rgba(17, 20, 24, 0.62);
  }

  .heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 28px;
  }

  .label {
    margin: 0 0 8px;
    color: #7d838c;
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  h2,
  h3 {
    margin: 0;
    letter-spacing: -0.02em;
  }

  h2 {
    font-size: 20px;
  }

  h3 {
    font-size: 16px;
  }

  .detail,
  .empty p,
  .node-placeholder {
    color: #858b95;
    line-height: 1.55;
  }

  .detail {
    max-width: 660px;
    margin: 8px 0 0;
  }

  .actions {
    flex: none;
    display: flex;
    align-items: end;
    gap: 8px;
  }

  .actions button,
  .source-actions button {
    border: 1px solid #d9dce1;
    border-radius: 7px;
    padding: 9px 12px;
    background: #f4f4f5;
    color: #111318;
    font: inherit;
    font-weight: 650;
    cursor: pointer;
  }

  .actions button:disabled,
  .source-actions button:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .max-nodes {
    display: grid;
    gap: 5px;
    color: #777f89;
    font-size: 10px;
  }

  .max-nodes input {
    width: 128px;
    border: 1px solid #343941;
    border-radius: 6px;
    padding: 8px 9px;
    background: #0f1216;
    color: #e5e7eb;
    font: inherit;
    font-size: 12px;
  }

  .source-config {
    margin-top: 12px;
    padding: 12px 14px;
    border: 1px solid #29323a;
    border-radius: 8px;
    background: #11161b;
    display: grid;
    gap: 10px;
  }

  .source-fields {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .source-fields > div,
  .nested-chain {
    min-width: 0;
    display: grid;
    gap: 5px;
  }

  .source-fields span,
  .nested-chain > span {
    color: #737a84;
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .nested-chain textarea {
    width: 100%;
    box-sizing: border-box;
    resize: vertical;
    border: 1px solid #343941;
    border-radius: 6px;
    padding: 8px 9px;
    background: #0f1216;
    color: #e5e7eb;
    font: 10px "SFMono-Regular", Consolas, "Liberation Mono", monospace;
    line-height: 1.45;
  }

  .source-actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .source-note {
    margin: 0;
    color: #bca575;
    font-size: 10px;
  }

  .empty,
  .error {
    margin-top: 20px;
    padding: 16px;
    border: 1px solid #292e35;
    border-radius: 9px;
    background: #14171b;
  }

  .empty strong {
    display: block;
    font-size: 14px;
  }

  .empty p {
    margin: 7px 0 0;
  }

  .error {
    border-color: #493027;
    background: #1c1411;
    color: #d2a187;
    font-size: 12px;
  }

  .workspace-path {
    margin-top: 18px;
    padding: 12px 14px;
    border: 1px solid #292e35;
    border-radius: 8px;
    background: #111419;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr);
    gap: 7px 12px;
    align-items: baseline;
  }

  .workspace-path span {
    color: #737a84;
    font-size: 10px;
    text-transform: uppercase;
  }

  code {
    overflow-wrap: anywhere;
    color: #c5cad1;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
    font-size: 10px;
  }

  .summary-grid {
    margin-top: 12px;
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 8px;
  }

  .summary-grid article {
    min-height: 78px;
    padding: 12px;
    border: 1px solid #292e35;
    border-radius: 8px;
    background: #14171b;
    display: grid;
    align-content: space-between;
    gap: 10px;
  }

  .summary-grid span,
  .limits span,
  .node-details span,
  .transform > span,
  .collision span,
  .section-title {
    color: #737a84;
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .summary-grid strong {
    font-size: 19px;
  }

  .limits {
    margin-top: 8px;
    padding: 10px 12px;
    border: 1px solid #29322d;
    border-radius: 8px;
    background: #111713;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .limits.truncated {
    border-color: #493b25;
    background: #19150e;
  }

  .limits > div {
    display: grid;
    gap: 4px;
  }

  .limits strong {
    font-size: 12px;
  }

  .warnings {
    margin-top: 8px;
    padding: 10px 12px;
    border: 1px solid #493b25;
    border-radius: 8px;
    background: #19150e;
    color: #bca575;
  }

  .warnings strong {
    font-size: 11px;
  }

  .warnings p {
    margin: 5px 0 0;
    font-size: 10px;
  }

  .preview-status {
    margin-top: 8px;
    padding: 10px 12px;
    border: 1px solid #29323a;
    border-radius: 8px;
    background: #11161b;
    display: grid;
    grid-template-columns: repeat(5, minmax(0, 1fr));
    gap: 9px;
  }

  .preview-status.loading {
    border-color: #3b4651;
  }

  .preview-status > div {
    min-width: 0;
    display: grid;
    gap: 4px;
  }

  .preview-status span {
    color: #737a84;
    font-size: 9px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .preview-status strong {
    overflow-wrap: anywhere;
    font-size: 11px;
  }

  .viewport-switcher {
    margin-top: 10px;
    padding: 9px 10px;
    border: 1px solid #29323a;
    border-radius: 8px;
    background: #10151a;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .viewport-switcher > div:first-child {
    display: grid;
    gap: 3px;
  }

  .viewport-switcher span {
    color: #727a84;
    font-size: 9px;
    font-weight: 650;
    text-transform: uppercase;
  }

  .viewport-switcher strong {
    color: #b8c0c9;
    font-size: 11px;
  }

  .viewport-switcher-actions {
    display: flex;
    gap: 6px;
  }

  .viewport-switcher-actions button {
    border: 1px solid #30363f;
    border-radius: 5px;
    background: #181c22;
    color: #858c96;
    padding: 5px 8px;
    font: inherit;
    font-size: 10px;
    cursor: pointer;
  }

  .viewport-switcher-actions button.active {
    border-color: #536575;
    background: #202933;
    color: #c4d0dc;
  }

  .native-viewport-warning {
    margin-top: 8px;
    padding: 8px 10px;
    border: 1px solid #4b3826;
    border-radius: 7px;
    background: #1b1510;
    color: #c2a27f;
    display: flex;
    gap: 8px;
    font-size: 10px;
  }

  .native-viewport-warning strong {
    color: #d0ad84;
  }

  .scene-layout {
    margin-top: 12px;
    display: grid;
    grid-template-columns: minmax(0, 1fr) 300px;
    gap: 12px;
    align-items: stretch;
  }

  .node-panel {
    min-width: 0;
    max-height: 618px;
    padding: 14px;
    border: 1px solid #292e35;
    border-radius: 10px;
    background: #111419;
    overflow: auto;
  }

  .node-panel-heading {
    display: flex;
    justify-content: space-between;
    gap: 12px;
    align-items: flex-start;
  }

  .node-panel-heading > span {
    color: #777f89;
    font-size: 10px;
  }

  .node-details {
    margin-top: 14px;
    display: grid;
    gap: 7px;
  }

  .node-details > div {
    padding: 8px;
    border: 1px solid #292e35;
    border-radius: 6px;
    background: #0e1115;
    display: grid;
    gap: 4px;
  }

  [data-state="resolved"],
  [data-state="localOnly"] {
    color: #8db79b;
  }

  [data-state="unresolved"] {
    color: #b47c72;
  }

  .transform,
  .collision,
  .diagnostic {
    margin-top: 9px;
    padding: 10px;
    border: 1px solid #292e35;
    border-radius: 7px;
    background: #0e1115;
    display: grid;
    gap: 5px;
  }

  .diagnostic {
    color: #9299a3;
    font-size: 11px;
  }

  .diagnostic.unresolved {
    border-color: #493027;
    background: #1a1210;
    color: #c2917e;
  }

  .diagnostic.preview-failure {
    border-color: #493b25;
    background: #19150e;
    color: #bea578;
  }

  .diagnostic.unresolved strong,
  .diagnostic.preview-failure strong {
    font-size: 10px;
  }

  .collision > div {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .collision > span {
    text-transform: none;
    line-height: 1.45;
  }

  .node-placeholder {
    margin: 14px 0 0;
    font-size: 11px;
  }

  .unresolved-list {
    margin-top: 16px;
    display: grid;
    gap: 5px;
  }

  .unresolved-list button {
    width: 100%;
    border: 1px solid #292e35;
    border-radius: 6px;
    padding: 7px 8px;
    background: #0e1115;
    color: #aeb4bc;
    text-align: left;
    cursor: pointer;
    display: grid;
    gap: 3px;
  }

  .unresolved-list button:hover,
  .unresolved-list button.selected {
    border-color: #46505b;
    background: #171b20;
  }

  .unresolved-list button span {
    overflow-wrap: anywhere;
    font-size: 10px;
  }

  .unresolved-list small {
    color: #a16f69;
    font-size: 9px;
  }

  .unresolved-list p {
    margin: 4px 0 0;
    color: #777f89;
    font-size: 9px;
  }

  .scene-empty {
    margin-top: 12px;
  }

  @media (max-width: 980px) {
    .summary-grid {
      grid-template-columns: repeat(3, minmax(0, 1fr));
    }

    .scene-layout {
      grid-template-columns: 1fr;
    }

    .node-panel {
      max-height: none;
    }
  }

  @media (max-width: 700px) {
    .heading {
      display: grid;
    }

    .actions {
      justify-content: start;
      flex-wrap: wrap;
    }

    .summary-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }

    .limits,
    .preview-status {
      grid-template-columns: 1fr;
    }

    .workspace-path {
      grid-template-columns: 1fr;
    }
  }
</style>
