<script lang="ts">
  import SceneViewport from "$lib/components/SceneViewport.svelte";
  import {
    assembleWorkspaceScene,
    chooseWorkspaceYmap,
    type AssetPreviewReport,
    type SceneManifestReport,
    type SceneNodeReport,
  } from "$lib/native";
  import {
    loadSceneAssetPreviews,
    type ScenePreviewLoadResult,
  } from "$lib/scenePreviews";

  export let workspacePath: string | null = null;

  let activeWorkspacePath: string | null = workspacePath;
  let ymapPath: string | null = null;
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
  let sceneGeneration = 0;

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

  $: selectedNode =
    manifest && selectedNodeIndex !== null
      ? (manifest.nodes.find((node) => node.index === selectedNodeIndex) ?? null)
      : null;

  $: unresolvedNodes =
    manifest?.nodes.filter((node) => node.resolution === "unresolved") ?? [];

  $: collisionNodes =
    manifest?.nodes.filter((node) => node.collision !== null) ?? [];

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

  async function loadScene() {
    if (!workspacePath || !ymapPath) {
      return;
    }

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
      const assembled = await assembleWorkspaceScene(
        workspacePath,
        ymapPath,
        parsedMaxNodes,
      );

      if (generation !== sceneGeneration) {
        return;
      }

      manifest = assembled;
      loading = false;
      previewLoading = true;

      const loaded = await loadSceneAssetPreviews(assembled);
      if (generation !== sceneGeneration) {
        return;
      }

      assetPreviews = loaded.previews;
      previewLoad = loaded;
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

  function selectNode(node: SceneNodeReport) {
    selectedNodeIndex = node.index;
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
        and unresolved reasons come directly from RageLab core.
      </p>
    </div>

    <div class="actions">
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

      <button
        type="button"
        onclick={chooseYmap}
        disabled={!workspacePath || loading}
      >
        {loading ? "Assembling…" : "Open workspace YMAP"}
      </button>
    </div>
  </div>

  {#if !workspacePath}
    <div class="empty">
      <strong>No workspace selected</strong>
      <p>Open a workspace first. Scene assembly is scoped to that root.</p>
    </div>
  {:else}
    <div class="workspace-path">
      <span>Workspace</span>
      <code>{workspacePath}</code>
      {#if ymapPath}
        <span>YMAP</span>
        <code>{ymapPath}</code>
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

      <div class="scene-layout">
        <SceneViewport
          {manifest}
          previews={assetPreviews}
          bind:selectedNodeIndex
        />

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

  .actions button {
    border: 1px solid #d9dce1;
    border-radius: 7px;
    padding: 9px 12px;
    background: #f4f4f5;
    color: #111318;
    font: inherit;
    font-weight: 650;
    cursor: pointer;
  }

  .actions button:disabled {
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
