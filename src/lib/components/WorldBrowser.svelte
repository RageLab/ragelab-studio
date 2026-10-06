<script lang="ts">
  import { onMount, tick } from "svelte";

  import NativeWorldViewport from "$lib/components/NativeWorldViewport.svelte";
  import {
    clearNativeWorldOverlays,
    focusNativeViewportNode,
    isolateNativeViewportNode,
    searchNativeWorld,
    selectNativeViewportNode,
    setNativeViewportLayerVisibility,
    setNativeWorldWorkspaceOverlay,
    setNativeViewportNodeVisible,
    showAllNativeViewportNodes,
    type GtaRpfIndexLocator,
    type NativeViewportReport,
    type NativeWorldActiveEntity,
    type NativeWorldActiveMap,
    type NativeWorldSearchReport,
    type NativeWorldSearchResult,
    type SceneGameIndexSource,
  } from "$lib/native";

  export let gameIndex: SceneGameIndexSource;
  export let workspacePath: string | null = null;
  export let workspaceYmap: string | null = null;
  export let initialPosition: [number, number, number] = [215, -810, 30];

  type BrowserLocation = {
    label: string;
    position: [number, number, number];
  };

  let viewport: NativeWorldViewport;
  let report: NativeViewportReport | null = null;
  let selectedNodeIndex: number | null = null;
  let selectedMapHash: number | null = null;
  let selectedEntityIndex: number | null = null;
  let selectedSearchResult: NativeWorldSearchResult | null = null;
  let query = "";
  let searchReport: NativeWorldSearchReport | null = null;
  let searchLoading = false;
  let errorMessage = "";
  let expandedMaps: number[] = [];
  let showBaseGame = true;
  let showWorkspaceOverlays = true;
  let overlayMounted = false;
  let overlayLoading = false;
  let x = String(initialPosition[0]);
  let y = String(initialPosition[1]);
  let z = String(initialPosition[2]);
  let bookmarks: BrowserLocation[] = [];
  let recentLocations: BrowserLocation[] = [];

  $: activeChunks = report?.streaming?.activeChunks ?? [];
  $: selectedMap =
    selectedMapHash === null
      ? null
      : (activeChunks.find((map) => map.mapHash === selectedMapHash) ?? null);
  $: selectedEntity =
    selectedMap && selectedEntityIndex !== null
      ? (selectedMap.entities.find((entry) => entry.entity.index === selectedEntityIndex) ?? null)
      : null;

  $: if (selectedNodeIndex !== null) {
    const located = findByRenderNode(activeChunks, selectedNodeIndex);
    if (located) {
      selectedMapHash = located.map.mapHash;
      selectedEntityIndex = located.entity.entity.index;
    }
  }

  onMount(() => {
    bookmarks = readLocations("ragelab.world.bookmarks");
    recentLocations = readLocations("ragelab.world.recent");
  });

  function hashHex(value: number | null | undefined) {
    return value === null || value === undefined
      ? "—"
      : "0x" + value.toString(16).padStart(8, "0").toUpperCase();
  }

  function providerPath(locator: GtaRpfIndexLocator | null | undefined) {
    if (!locator) return "—";
    const nested = locator.nested.length > 0 ? " :: " + locator.nested.join(" :: ") : "";
    return locator.archiveRelative + nested + " :: " + locator.entry;
  }

  function mapLabel(map: NativeWorldActiveMap) {
    const entry = map.provider.entry.replace(/\\/g, "/");
    return entry.split("/").at(-1) ?? hashHex(map.mapHash);
  }

  function toggleMap(mapHash: number) {
    expandedMaps = expandedMaps.includes(mapHash)
      ? expandedMaps.filter((value) => value !== mapHash)
      : [...expandedMaps, mapHash];
  }

  async function selectEntity(map: NativeWorldActiveMap, entry: NativeWorldActiveEntity) {
    selectedMapHash = map.mapHash;
    selectedEntityIndex = entry.entity.index;
    selectedSearchResult = null;
    selectedNodeIndex = entry.renderNodeIndex;
    if (entry.renderNodeIndex !== null) {
      report = await selectNativeViewportNode(entry.renderNodeIndex);
    } else {
      report = await selectNativeViewportNode(null);
    }
  }

  async function focusSelection() {
    if (selectedEntity?.renderNodeIndex === null || selectedEntity?.renderNodeIndex === undefined) return;
    try {
      report = await focusNativeViewportNode(selectedEntity.renderNodeIndex);
      selectedNodeIndex = report.stats.selectedNodeIndex;
    } catch (error) {
      setError(error);
    }
  }

  async function hideSelection() {
    if (selectedEntity?.renderNodeIndex === null || selectedEntity?.renderNodeIndex === undefined) return;
    try {
      report = await setNativeViewportNodeVisible(selectedEntity.renderNodeIndex, false);
      selectedNodeIndex = report.stats.selectedNodeIndex;
    } catch (error) {
      setError(error);
    }
  }

  async function isolateSelection() {
    if (selectedEntity?.renderNodeIndex === null || selectedEntity?.renderNodeIndex === undefined) return;
    try {
      report = await isolateNativeViewportNode(selectedEntity.renderNodeIndex);
      selectedNodeIndex = report.stats.selectedNodeIndex;
    } catch (error) {
      setError(error);
    }
  }

  async function showAll() {
    try {
      report = await showAllNativeViewportNodes();
    } catch (error) {
      setError(error);
    }
  }

  async function updateLayerVisibility() {
    try {
      report = await setNativeViewportLayerVisibility(showBaseGame, showWorkspaceOverlays);
      selectedNodeIndex = report.stats.selectedNodeIndex;
    } catch (error) {
      setError(error);
    }
  }

  async function toggleWorkspaceOverlay() {
    if (overlayLoading) return;
    overlayLoading = true;
    errorMessage = "";
    try {
      if (overlayMounted) {
        report = await clearNativeWorldOverlays();
        overlayMounted = false;
      } else {
        if (!workspacePath || !workspaceYmap) {
          throw new Error("Open a workspace YMAP before mounting a local world overlay.");
        }
        report = await setNativeWorldWorkspaceOverlay({
          workspace: workspacePath,
          ymap: workspaceYmap,
          gameIndex,
        });
        overlayMounted = true;
      }
      selectedNodeIndex = report.stats.selectedNodeIndex;
    } catch (error) {
      setError(error);
    } finally {
      overlayLoading = false;
    }
  }

  async function runSearch() {
    const needle = query.trim();
    if (!needle) return;
    searchLoading = true;
    errorMessage = "";
    try {
      searchReport = await searchNativeWorld(needle, 80);
      selectedSearchResult = searchReport.results[0] ?? null;
    } catch (error) {
      setError(error);
    } finally {
      searchLoading = false;
    }
  }

  async function jumpToResult(result: NativeWorldSearchResult) {
    selectedSearchResult = result;
    const position = result.position
      ? [result.position.x, result.position.y, result.position.z] as [number, number, number]
      : result.bounds
        ? [
            (result.bounds.min.x + result.bounds.max.x) / 2,
            (result.bounds.min.y + result.bounds.max.y) / 2,
            (result.bounds.min.z + result.bounds.max.z) / 2,
          ] as [number, number, number]
        : null;
    if (!position) return;

    try {
      await viewport.teleport(position, true);
      rememberRecent(result.label, position);
      x = position[0].toFixed(2);
      y = position[1].toFixed(2);
      z = position[2].toFixed(2);
      await tick();

      if (result.mapHash !== null && result.entityIndex !== null) {
        const map = activeChunks.find((candidate) => candidate.mapHash === result.mapHash);
        const entity = map?.entities.find((candidate) => candidate.entity.index === result.entityIndex);
        if (map && entity) await selectEntity(map, entity);
      }
    } catch (error) {
      setError(error);
    }
  }

  async function teleportCoordinates() {
    const position = [Number(x), Number(y), Number(z)] as [number, number, number];
    if (!position.every(Number.isFinite)) {
      errorMessage = "Coordinates must be finite numbers.";
      return;
    }
    try {
      await viewport.teleport(position, true);
      rememberRecent("Coordinates", position);
    } catch (error) {
      setError(error);
    }
  }

  async function jumpLocation(location: BrowserLocation) {
    x = String(location.position[0]);
    y = String(location.position[1]);
    z = String(location.position[2]);
    try {
      await viewport.teleport(location.position, true);
      rememberRecent(location.label, location.position);
    } catch (error) {
      setError(error);
    }
  }

  function bookmarkCurrent() {
    const current = report?.streaming?.position;
    if (!current) return;
    const location: BrowserLocation = {
      label:
        selectedSearchResult?.label ??
        (selectedMap ? mapLabel(selectedMap) : "World location"),
      position: [current.x, current.y, current.z],
    };
    bookmarks = dedupeLocations([location, ...bookmarks]).slice(0, 24);
    writeLocations("ragelab.world.bookmarks", bookmarks);
  }

  function removeBookmark(index: number) {
    bookmarks = bookmarks.filter((_, current) => current !== index);
    writeLocations("ragelab.world.bookmarks", bookmarks);
  }

  async function findSelectedAsset() {
    const hash = selectedEntity?.resolution?.assetHash;
    if (hash === null || hash === undefined) return;
    query = hashHex(hash);
    await runSearch();
  }

  function rememberRecent(label: string, position: [number, number, number]) {
    recentLocations = dedupeLocations([{ label, position }, ...recentLocations]).slice(0, 12);
    writeLocations("ragelab.world.recent", recentLocations);
  }

  function readLocations(key: string): BrowserLocation[] {
    try {
      const parsed = JSON.parse(localStorage.getItem(key) ?? "[]");
      if (!Array.isArray(parsed)) return [];
      return parsed
        .filter((entry) =>
          entry &&
          typeof entry.label === "string" &&
          Array.isArray(entry.position) &&
          entry.position.length === 3 &&
          entry.position.every((value: unknown) => typeof value === "number" && Number.isFinite(value)),
        )
        .map((entry) => ({
          label: String(entry.label),
          position: [
            Number(entry.position[0]),
            Number(entry.position[1]),
            Number(entry.position[2]),
          ] as [number, number, number],
        }))
        .slice(0, 24);
    } catch {
      return [];
    }
  }

  function writeLocations(key: string, locations: BrowserLocation[]) {
    localStorage.setItem(key, JSON.stringify(locations));
  }

  function dedupeLocations(locations: BrowserLocation[]) {
    const seen = new Set<string>();
    return locations.filter((location) => {
      const key = location.position.map((value) => value.toFixed(2)).join(",");
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
  }

  function findByRenderNode(chunks: NativeWorldActiveMap[], nodeIndex: number) {
    for (const map of chunks) {
      const entity = map.entities.find((entry) => entry.renderNodeIndex === nodeIndex);
      if (entity) return { map, entity };
    }
    return null;
  }

  function setError(error: unknown) {
    errorMessage = error instanceof Error ? error.message : String(error ?? "World browser failed.");
  }
</script>

<section class="world-browser" id="world-browser">
  <div class="browser-heading">
    <div>
      <p class="label">GTA Legacy world browser</p>
      <h2>Camera-driven world</h2>
      <p class="detail">
        Search, hierarchy, provenance and inspection come from the persisted Rust world index.
        Only nearby YMAP chunks are decoded and resident in the native wgpu renderer.
      </p>
    </div>
    <div class="layer-controls">
      <label><input type="checkbox" bind:checked={showBaseGame} onchange={updateLayerVisibility} /> GTA base</label>
      <label><input type="checkbox" bind:checked={showWorkspaceOverlays} onchange={updateLayerVisibility} /> Workspace/FiveM overlays</label>
      {#if workspaceYmap}
        <button type="button" class:active={overlayMounted} onclick={() => void toggleWorkspaceOverlay()} disabled={overlayLoading}>
          {overlayLoading ? "Applying…" : overlayMounted ? "Clear workspace overlay" : "Mount workspace overlay"}
        </button>
      {/if}
      {#if report?.streaming?.overlayPackages}
        <span class="overlay-count">{report.streaming.overlayPackages} local overlay package(s)</span>
      {/if}
    </div>
  </div>

  <div class="toolbar">
    <form onsubmit={(event) => { event.preventDefault(); void runSearch(); }}>
      <input bind:value={query} placeholder="Search YMAP, archetype, asset, hash (0x…)" aria-label="World search" />
      <button type="submit" disabled={searchLoading}>{searchLoading ? "Searching…" : "Search"}</button>
    </form>
    <div class="coordinates">
      <input bind:value={x} aria-label="World X" />
      <input bind:value={y} aria-label="World Y" />
      <input bind:value={z} aria-label="World Z" />
      <button type="button" onclick={() => void teleportCoordinates()}>Teleport</button>
      <button type="button" onclick={bookmarkCurrent}>Bookmark</button>
    </div>
  </div>

  {#if errorMessage}<div class="error" role="alert">{errorMessage}</div>{/if}

  <div class="browser-grid">
    <aside class="tree-panel">
      <div class="panel-title">
        <div><span>Loaded hierarchy</span><strong>{activeChunks.length} YMAPs</strong></div>
        <button type="button" onclick={() => void showAll()}>Show all</button>
      </div>

      {#if activeChunks.length === 0}
        <p class="empty">No streamed maps at the current camera position.</p>
      {:else}
        <div class="map-tree">
          {#each activeChunks as map (map.mapHash + ":" + map.provider.loadRank)}
            <div class="map-entry">
              <button class="map-row" type="button" onclick={() => toggleMap(map.mapHash)}>
                <span>{expandedMaps.includes(map.mapHash) ? "▾" : "▸"}</span>
                <strong>{mapLabel(map)}</strong>
                <code>{hashHex(map.mapHash)}</code>
                <em>GTA base</em>
              </button>
              {#if expandedMaps.includes(map.mapHash)}
                <div class="entity-list">
                  {#each map.entities.slice(0, 300) as entry (entry.entity.index)}
                    <button
                      type="button"
                      class:selected={selectedMapHash === map.mapHash && selectedEntityIndex === entry.entity.index}
                      onclick={() => void selectEntity(map, entry)}
                    >
                      <span>#{entry.entity.index}</span>
                      <code>{hashHex(entry.entity.archetypeHash)}</code>
                      <small>{entry.resolution?.assetKind ?? "unresolved"} · {entry.renderNodeIndex === null ? "not rendered" : "node " + entry.renderNodeIndex}</small>
                    </button>
                  {/each}
                  {#if map.entities.length > 300}
                    <p class="limit-note">{map.entities.length - 300} more entities omitted from this UI list.</p>
                  {/if}
                </div>
              {/if}
            </div>
          {/each}
        </div>
      {/if}

      {#if searchReport}
        <div class="search-results">
          <div class="section-heading">
            <span>Global search</span>
            <strong>{searchReport.results.length}{searchReport.truncated ? "+" : ""}</strong>
          </div>
          {#each searchReport.results as result, index (result.kind + ":" + result.hash + ":" + index)}
            <button
              type="button"
              class:selected={selectedSearchResult === result}
              onclick={() => { selectedSearchResult = result; void jumpToResult(result); }}
            >
              <div><strong>{result.label}</strong><em>{result.kind}</em></div>
              <code>{hashHex(result.hash)}</code>
              <small>{providerPath(result.provider)}</small>
            </button>
          {/each}
        </div>
      {/if}
    </aside>

    <div class="viewport-column">
      <NativeWorldViewport
        bind:this={viewport}
        {gameIndex}
        {initialPosition}
        bind:report
        bind:selectedNodeIndex
        onNativeFailure={(message) => errorMessage = message}
      />

      <div class="locations">
        <div>
          <div class="section-heading"><span>Bookmarks</span><strong>{bookmarks.length}</strong></div>
          {#if bookmarks.length === 0}<p class="empty">No bookmarks yet.</p>{/if}
          {#each bookmarks.slice(0, 8) as location, index}
            <div class="location-row">
              <button type="button" onclick={() => void jumpLocation(location)}>{location.label}</button>
              <code>{location.position.map((value) => value.toFixed(1)).join(", ")}</code>
              <button class="remove" type="button" onclick={() => removeBookmark(index)}>×</button>
            </div>
          {/each}
        </div>
        <div>
          <div class="section-heading"><span>Recent</span><strong>{recentLocations.length}</strong></div>
          {#if recentLocations.length === 0}<p class="empty">No recent jumps.</p>{/if}
          {#each recentLocations.slice(0, 8) as location}
            <div class="location-row">
              <button type="button" onclick={() => void jumpLocation(location)}>{location.label}</button>
              <code>{location.position.map((value) => value.toFixed(1)).join(", ")}</code>
            </div>
          {/each}
        </div>
      </div>
    </div>

    <aside class="inspector">
      <div class="panel-title">
        <div><span>Inspector</span><strong>{selectedEntity ? "Entity #" + selectedEntity.entity.index : selectedSearchResult ? selectedSearchResult.kind : "No selection"}</strong></div>
      </div>

      {#if selectedEntity && selectedMap}
        <div class="badges"><span>GTA base</span><span>{mapLabel(selectedMap)}</span></div>
        <dl>
          <dt>YMAP</dt><dd><code>{hashHex(selectedMap.mapHash)}</code></dd>
          <dt>Entity</dt><dd>{selectedEntity.entity.index}</dd>
          <dt>Archetype</dt><dd><code>{hashHex(selectedEntity.entity.archetypeHash)}</code></dd>
          <dt>Position</dt><dd><code>{selectedEntity.entity.position.x.toFixed(3)}, {selectedEntity.entity.position.y.toFixed(3)}, {selectedEntity.entity.position.z.toFixed(3)}</code></dd>
          <dt>Rotation</dt><dd><code>{selectedEntity.entity.rotation.map((value) => value.toFixed(5)).join(", ")}</code></dd>
          <dt>Scale XY/Z</dt><dd>{selectedEntity.entity.scaleXy ?? "—"} / {selectedEntity.entity.scaleZ ?? "—"}</dd>
          <dt>Flags</dt><dd><code>{selectedEntity.entity.flags}</code></dd>
          <dt>Parent index</dt><dd>{selectedEntity.entity.parentIndex ?? "—"}</dd>
          <dt>Render node</dt><dd>{selectedEntity.renderNodeIndex ?? "not rendered"}</dd>
          <dt>Archetype provider</dt><dd><code>{providerPath(selectedEntity.resolution?.provider)}</code></dd>
          <dt>Asset</dt><dd>{selectedEntity.resolution?.assetKind ?? "—"} <code>{hashHex(selectedEntity.resolution?.assetHash)}</code></dd>
          <dt>Asset provider</dt><dd><code>{providerPath(selectedEntity.resolution?.assetProvider)}</code></dd>
          <dt>Texture dictionary</dt><dd><code>{hashHex(selectedEntity.resolution?.textureDictionaryHash)}</code></dd>
          <dt>Physics dictionary</dt><dd><code>{hashHex(selectedEntity.resolution?.physicsDictionaryHash)}</code></dd>
        </dl>
        <div class="entity-actions">
          <button type="button" disabled={selectedEntity.renderNodeIndex === null} onclick={() => void focusSelection()}>Focus</button>
          <button type="button" disabled={selectedEntity.renderNodeIndex === null} onclick={() => void isolateSelection()}>Isolate</button>
          <button type="button" disabled={selectedEntity.renderNodeIndex === null} onclick={() => void hideSelection()}>Hide</button>
          <button type="button" disabled={!selectedEntity.resolution?.assetHash} onclick={() => void findSelectedAsset()}>Find provider asset</button>
        </div>
      {:else if selectedSearchResult}
        <div class="badges"><span>{selectedSearchResult.kind}</span><span>GTA index</span></div>
        <dl>
          <dt>Label</dt><dd>{selectedSearchResult.label}</dd>
          <dt>Hash</dt><dd><code>{hashHex(selectedSearchResult.hash)}</code></dd>
          <dt>Provider</dt><dd><code>{providerPath(selectedSearchResult.provider)}</code></dd>
          <dt>Asset kind</dt><dd>{selectedSearchResult.assetKind ?? "—"}</dd>
          <dt>Asset hash</dt><dd><code>{hashHex(selectedSearchResult.assetHash)}</code></dd>
          <dt>Asset provider</dt><dd><code>{providerPath(selectedSearchResult.assetProvider)}</code></dd>
          <dt>YMAP</dt><dd><code>{hashHex(selectedSearchResult.mapHash)}</code></dd>
          <dt>Entity</dt><dd>{selectedSearchResult.entityIndex ?? "—"}</dd>
        </dl>
        <button type="button" disabled={!selectedSearchResult.position && !selectedSearchResult.bounds} onclick={() => void jumpToResult(selectedSearchResult!)}>Jump / teleport</button>
      {:else}
        <p class="empty">Pick an entity, choose a hierarchy row, or select a global search result.</p>
      {/if}

      <div class="resource-explorer">
        <div class="section-heading"><span>Resource explorer</span></div>
        {#if workspacePath}
          <div class="resource-row"><span>Workspace</span><code>{workspacePath}</code><em>editable local</em></div>
          {#if workspaceYmap}<div class="resource-row"><span>YMAP</span><code>{workspaceYmap}</code><em>workspace</em></div>{/if}
        {:else}
          <p class="empty">No workspace mounted.</p>
        {/if}
        {#if selectedEntity?.resolution?.assetProvider}
          <div class="resource-row"><span>Resolved GTA asset</span><code>{providerPath(selectedEntity.resolution.assetProvider)}</code><em>read-only</em></div>
        {/if}
        {#if selectedSearchResult}
          <div class="resource-row"><span>Search provider</span><code>{providerPath(selectedSearchResult.provider)}</code><em>read-only</em></div>
        {/if}
      </div>
    </aside>
  </div>
</section>

<style>
  .world-browser { margin-top: 18px; padding: 18px; border: 1px solid #29313a; border-radius: 12px; background: #0f1216; }
  .browser-heading,.toolbar,.panel-title,.section-heading,.location-row,.map-row,.search-results button > div,.layer-controls,.entity-actions,.badges { display:flex; align-items:center; gap:8px; }
  .browser-heading { justify-content:space-between; align-items:flex-start; gap:24px; }
  .label { margin:0 0 6px; color:#7d838c; font-size:10px; font-weight:700; letter-spacing:.05em; text-transform:uppercase; }
  h2 { margin:0; font-size:20px; }
  .detail { margin:8px 0 0; max-width:720px; color:#818894; font-size:11px; line-height:1.55; }
  .layer-controls { flex-wrap:wrap; justify-content:flex-end; color:#969da7; font-size:10px; }
  .layer-controls label { display:flex; align-items:center; gap:5px; white-space:nowrap; }
  .toolbar { margin-top:14px; justify-content:space-between; flex-wrap:wrap; }
  .toolbar form,.coordinates { display:flex; gap:6px; align-items:center; }
  .toolbar form { flex:1; min-width:320px; }
  .toolbar form input { flex:1; }
  .coordinates input { width:78px; }
  input,button { border:1px solid #30363f; border-radius:5px; background:#171b20; color:#c6ccd4; padding:6px 8px; font:inherit; font-size:10px; }
  button { cursor:pointer; }
  button:hover:not(:disabled),button.selected { border-color:#556371; background:#20262d; }
  button:disabled { opacity:.45; cursor:default; }
  .error { margin-top:10px; padding:8px 10px; border:1px solid #54362d; border-radius:6px; background:#1b1210; color:#d2a187; font-size:10px; }
  .browser-grid { margin-top:12px; display:grid; grid-template-columns:260px minmax(420px,1fr) 300px; gap:10px; align-items:start; }
  .tree-panel,.inspector { min-height:560px; max-height:760px; overflow:auto; padding:10px; border:1px solid #292e35; border-radius:8px; background:#111419; }
  .panel-title,.section-heading { justify-content:space-between; }
  .panel-title > div,.section-heading { color:#78808a; font-size:9px; text-transform:uppercase; }
  .panel-title strong,.section-heading strong { color:#aeb5be; }
  .map-tree,.entity-list,.search-results,.resource-explorer,.locations > div { display:grid; gap:4px; }
  .map-tree,.search-results,.resource-explorer { margin-top:10px; }
  .map-entry { border:1px solid #272c33; border-radius:6px; overflow:hidden; background:#0e1115; }
  .map-row { width:100%; border:0; border-radius:0; display:grid; grid-template-columns:14px minmax(0,1fr); text-align:left; }
  .map-row strong,.map-row code,.map-row em { grid-column:2; overflow-wrap:anywhere; }
  .map-row code,.entity-list code,.search-results code,dl code,.location-row code,.resource-row code { font-family:"SFMono-Regular",Consolas,monospace; font-size:9px; color:#aeb5bd; overflow-wrap:anywhere; }
  .map-row em,.badges span,.resource-row em { width:max-content; padding:2px 5px; border:1px solid #365044; border-radius:9px; color:#85ad94; font-size:8px; font-style:normal; }
  .entity-list { padding:5px; border-top:1px solid #272c33; }
  .entity-list button,.search-results button { width:100%; text-align:left; display:grid; gap:2px; padding:6px; }
  .entity-list small,.search-results small { color:#69717b; font-size:8px; overflow-wrap:anywhere; }
  .search-results { padding-top:10px; border-top:1px solid #292e35; }
  .search-results button > div { justify-content:space-between; }
  .search-results em { color:#7e8791; font-size:8px; font-style:normal; text-transform:uppercase; }
  .viewport-column { min-width:0; }
  .locations { margin-top:10px; display:grid; grid-template-columns:1fr 1fr; gap:8px; }
  .locations > div { padding:8px; border:1px solid #292e35; border-radius:7px; background:#111419; }
  .location-row { min-width:0; }
  .location-row button:first-child { flex:1; min-width:0; text-align:left; overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
  .location-row code { flex:none; }
  .location-row .remove { padding:3px 6px; }
  .inspector dl { margin:10px 0 0; display:grid; grid-template-columns:94px minmax(0,1fr); gap:6px 8px; font-size:9px; }
  .inspector dt { color:#727a84; text-transform:uppercase; }
  .inspector dd { margin:0; color:#b8bec6; overflow-wrap:anywhere; }
  .badges { margin-top:10px; flex-wrap:wrap; }
  .entity-actions { margin-top:10px; flex-wrap:wrap; }
  .resource-explorer { margin-top:14px; padding-top:12px; border-top:1px solid #292e35; }
  .resource-row { padding:7px; border:1px solid #252a31; border-radius:5px; background:#0d1014; display:grid; gap:4px; }
  .resource-row span { color:#777f89; font-size:8px; text-transform:uppercase; }
  .empty,.limit-note { margin:8px 0 0; color:#6e7680; font-size:9px; line-height:1.45; }
  @media (max-width:1200px) { .browser-grid { grid-template-columns:230px minmax(0,1fr); } .inspector { grid-column:1 / -1; min-height:0; max-height:none; } }
  @media (max-width:800px) { .browser-grid,.locations { grid-template-columns:1fr; } .tree-panel,.inspector { min-height:0; max-height:none; } .browser-heading { display:grid; } .layer-controls { justify-content:flex-start; } .toolbar form { min-width:100%; } }
</style>
