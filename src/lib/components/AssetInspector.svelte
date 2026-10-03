<script lang="ts">
  import PreviewViewport from "$lib/components/PreviewViewport.svelte";
  import {
    chooseAssetFile,
    getAssetCapabilities,
    inspectAsset,
    previewAsset,
    type AssetCapabilitiesReport,
    type AssetInspectionReport,
    type AssetOperationAvailability,
    type AssetOperationCapability,
    type AssetPreviewReport,
    type CorePreviewOptions,
  } from "$lib/native";

  let inspection: AssetInspectionReport | null = null;
  let capabilities: AssetCapabilitiesReport | null = null;
  let loading = false;
  let errorMessage = "";

  let previewReport: AssetPreviewReport | null = null;
  let previewLoading = false;
  let previewError = "";
  let drawableIndex = "0";
  let previewOperation: AssetOperationCapability | null = null;
  let previewRequiresDrawableIndex = false;
  let previewRunnable = false;

  $: previewOperation =
    capabilities?.operations.find((operation) => operation.id === "preview") ??
    null;
  $: previewRequiresDrawableIndex =
    previewOperation?.requiresParameters.includes("drawableIndex") ?? false;
  $: previewRunnable =
    previewOperation?.availability === "available" ||
    previewOperation?.availability === "parameterized";

  async function openAsset() {
    errorMessage = "";

    let path: string | null;
    try {
      path = await chooseAssetFile();
    } catch (error) {
      errorMessage = errorMessageFor(error, "Unable to open asset picker.");
      return;
    }

    if (!path) {
      return;
    }

    loading = true;
    inspection = null;
    capabilities = null;
    previewReport = null;
    previewError = "";
    drawableIndex = "0";

    try {
      [inspection, capabilities] = await Promise.all([
        inspectAsset(path),
        getAssetCapabilities(path),
      ]);
    } catch (error) {
      inspection = null;
      capabilities = null;
      errorMessage = errorMessageFor(
        error,
        "RageLab core could not inspect this asset.",
      );
    } finally {
      loading = false;
    }
  }

  async function loadPreview() {
    if (!inspection || !previewOperation || !previewRunnable) {
      return;
    }

    previewError = "";
    previewLoading = true;
    previewReport = null;

    const options: CorePreviewOptions = {};

    if (previewRequiresDrawableIndex) {
      const selectedIndex = Number(drawableIndex);
      if (!Number.isInteger(selectedIndex) || selectedIndex < 0) {
        previewError = "drawableIndex must be a non-negative integer.";
        previewLoading = false;
        return;
      }
      options.drawableIndex = selectedIndex;
    }

    try {
      previewReport = await previewAsset(inspection.path, options);
    } catch (error) {
      previewError = errorMessageFor(
        error,
        "RageLab core could not build a preview for this asset.",
      );
    } finally {
      previewLoading = false;
    }
  }

  function errorMessageFor(error: unknown, fallback: string): string {
    return error instanceof Error && error.message ? error.message : fallback;
  }

  function formatBytes(bytes: number): string {
    if (bytes < 1024) {
      return bytes + " B";
    }
    if (bytes < 1024 * 1024) {
      return (bytes / 1024).toFixed(1) + " KiB";
    }
    return (bytes / (1024 * 1024)).toFixed(1) + " MiB";
  }

  function availabilityLabel(value: AssetOperationAvailability): string {
    switch (value) {
      case "available":
        return "Available";
      case "parameterized":
        return "Parameters required";
      case "contextRequired":
        return "Workspace required";
      case "unavailable":
        return "Unavailable";
    }
  }
</script>

<section class="asset-inspector" id="asset-inspector" aria-live="polite">
  <div class="heading">
    <div>
      <p class="label">Asset inspector</p>
      <h2>Core-backed inspection</h2>
      <p class="detail">
        Metadata and operation eligibility come directly from the pinned RageLab
        Rust core.
      </p>
    </div>

    <button class="open-asset" type="button" onclick={openAsset} disabled={loading}>
      {loading ? "Inspecting…" : "Open asset"}
    </button>
  </div>

  {#if errorMessage}
    <div class="error" role="alert">
      <strong>Inspection failed</strong>
      <span>{errorMessage}</span>
    </div>
  {:else if inspection && capabilities}
    <div class="asset-path">
      <span class="muted">Selected asset</span>
      <code>{inspection.path}</code>
    </div>

    <div class="summary-grid">
      <article>
        <span class="muted">Type</span>
        <strong>{inspection.type}</strong>
      </article>
      <article>
        <span class="muted">Size</span>
        <strong>{formatBytes(inspection.bytes)}</strong>
      </article>
      <article>
        <span class="muted">Container</span>
        <strong>{inspection.container?.kind ?? "None"}</strong>
      </article>
      <article>
        <span class="muted">Resource version</span>
        <strong>{inspection.container?.version ?? "—"}</strong>
      </article>
    </div>

    {#if inspection.container}
      <div class="container-meta">
        <span>System {formatBytes(inspection.container.systemSize)}</span>
        <span>Graphics {formatBytes(inspection.container.graphicsSize)}</span>
        <span>
          {inspection.container.decompression
            ? "Container decompressed"
            : "Container not decompressed"}
        </span>
      </div>
    {/if}

    <details class="metadata">
      <summary>Structured metadata</summary>
      <pre>{JSON.stringify(inspection.details, null, 2)}</pre>
    </details>

    {#if previewOperation}
      <section class="preview-controls">
        <div>
          <p class="label">Preview capability</p>
          <div class="preview-capability-line">
            <code>{previewOperation.id}</code>
            <span
              class="availability"
              data-availability={previewOperation.availability}
            >
              {availabilityLabel(previewOperation.availability)}
            </span>
          </div>

          {#if previewOperation.reason}
            <p class="preview-reason">{previewOperation.reason}</p>
          {/if}
        </div>

        <div class="preview-actions">
          {#if previewRequiresDrawableIndex}
            <label class="selector-field">
              <span>drawableIndex</span>
              <input
                type="number"
                min="0"
                step="1"
                bind:value={drawableIndex}
                disabled={previewLoading}
              />
            </label>
          {/if}

          <button
            class="preview-button"
            type="button"
            onclick={loadPreview}
            disabled={!previewRunnable || previewLoading}
          >
            {previewLoading ? "Building preview…" : "Preview asset"}
          </button>
        </div>
      </section>

      {#if previewError}
        <div class="preview-error" role="alert">
          <strong>Preview failed</strong>
          <span>{previewError}</span>
        </div>
      {/if}

      {#if previewReport}
        <PreviewViewport report={previewReport} />
      {/if}
    {/if}

    <div class="operations-heading">
      <div>
        <p class="label">Capabilities</p>
        <h3>{capabilities.operations.length} reported operation(s)</h3>
      </div>
      <span class="muted">Eligibility is evaluated by RageLab core</span>
    </div>

    <div class="operations">
      {#each capabilities.operations as operation}
        <article class="operation">
          <div class="operation-topline">
            <code>{operation.id}</code>
            <span class:write={operation.writesAsset} class="mode">
              {operation.writesAsset ? "write" : "read"}
            </span>
          </div>

          <span class="availability" data-availability={operation.availability}>
            {availabilityLabel(operation.availability)}
          </span>

          {#if operation.reason}
            <p>{operation.reason}</p>
          {/if}

          {#if operation.requiresParameters.length > 0}
            <div class="parameters">
              {#each operation.requiresParameters as parameter}
                <code>{parameter}</code>
              {/each}
            </div>
          {/if}
        </article>
      {/each}
    </div>
  {:else}
    <div class="empty">
      <strong>No asset selected</strong>
      <p>
        Choose any local file. RageLab core determines its type and which
        operations, if any, are valid for that exact asset.
      </p>
    </div>
  {/if}
</section>

<style>
  .asset-inspector {
    margin-top: 18px;
    padding: 24px;
    border: 1px solid #25292f;
    border-radius: 12px;
    background: rgba(17, 20, 24, 0.62);
  }

  .heading,
  .operations-heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 28px;
  }

  .label {
    margin: 0 0 8px;
    color: #7d838c;
    font-size: 12px;
    font-weight: 600;
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
  .operation p {
    color: #858b95;
    line-height: 1.55;
  }

  .detail {
    max-width: 620px;
    margin: 8px 0 0;
  }

  .open-asset {
    flex: none;
    border: 1px solid #d9dce1;
    border-radius: 7px;
    padding: 9px 13px;
    background: #f4f4f5;
    color: #111318;
    font: inherit;
    font-weight: 650;
    cursor: pointer;
  }

  .open-asset:hover:not(:disabled) {
    background: #ffffff;
  }

  .open-asset:disabled {
    cursor: default;
    opacity: 0.6;
  }

  .empty,
  .error {
    margin-top: 22px;
    padding: 18px;
    border: 1px solid #292e35;
    border-radius: 9px;
    background: #14171b;
  }

  .empty strong,
  .error strong {
    display: block;
    font-size: 14px;
  }

  .empty p {
    max-width: 720px;
    margin: 8px 0 0;
  }

  .error {
    border-color: #493227;
    color: #e1b99c;
  }

  .error span {
    display: block;
    margin-top: 7px;
    color: #b8957d;
    font-size: 13px;
    overflow-wrap: anywhere;
  }

  .asset-path {
    margin-top: 22px;
    padding: 14px 16px;
    border: 1px solid #292e35;
    border-radius: 9px;
    background: #111419;
    display: grid;
    gap: 7px;
  }

  code {
    overflow-wrap: anywhere;
    color: #e7e8ea;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
    font-size: 12px;
  }

  .muted {
    color: #7f8690;
    font-size: 12px;
  }

  .summary-grid {
    margin-top: 12px;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .summary-grid article {
    min-height: 84px;
    padding: 15px;
    border: 1px solid #292e35;
    border-radius: 9px;
    background: #14171b;
    display: grid;
    align-content: space-between;
    gap: 12px;
  }

  .summary-grid strong {
    font-size: 17px;
    overflow-wrap: anywhere;
  }

  .container-meta {
    margin-top: 10px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .container-meta span {
    padding: 6px 8px;
    border: 1px solid #292e35;
    border-radius: 6px;
    color: #7f8690;
    background: #12151a;
    font-size: 11px;
  }

  .metadata {
    margin-top: 12px;
    border: 1px solid #292e35;
    border-radius: 9px;
    background: #111419;
  }

  .metadata summary {
    padding: 12px 14px;
    color: #c5c9cf;
    font-size: 13px;
    cursor: pointer;
  }

  .metadata pre {
    max-height: 320px;
    margin: 0;
    padding: 0 14px 14px;
    overflow: auto;
    color: #aeb4bd;
    white-space: pre-wrap;
    overflow-wrap: anywhere;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
    font-size: 11px;
    line-height: 1.5;
  }

  .preview-controls {
    margin-top: 14px;
    padding: 14px;
    border: 1px solid #292e35;
    border-radius: 9px;
    background: #12151a;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
  }

  .preview-capability-line {
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .preview-reason {
    max-width: 620px;
    margin: 7px 0 0;
    color: #858b95;
    font-size: 12px;
    line-height: 1.5;
  }

  .preview-actions {
    flex: none;
    display: flex;
    align-items: end;
    gap: 8px;
  }

  .selector-field {
    display: grid;
    gap: 5px;
    color: #7f8690;
    font-size: 10px;
  }

  .selector-field input {
    width: 92px;
    border: 1px solid #343941;
    border-radius: 6px;
    padding: 8px 9px;
    background: #0f1216;
    color: #e5e7eb;
    font: inherit;
  }

  .preview-button {
    border: 1px solid #343941;
    border-radius: 7px;
    padding: 8px 11px;
    background: #1a1e24;
    color: #dce0e5;
    font: inherit;
    font-weight: 650;
    cursor: pointer;
  }

  .preview-button:hover:not(:disabled) {
    border-color: #4b525d;
    background: #20252c;
  }

  .preview-button:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .preview-error {
    margin-top: 10px;
    padding: 11px 13px;
    border: 1px solid #493027;
    border-radius: 8px;
    background: #1c1411;
    color: #d2a187;
    font-size: 12px;
  }

  .preview-error strong {
    margin-right: 8px;
  }

  .operations-heading {
    margin-top: 28px;
    align-items: end;
  }

  .operations {
    margin-top: 12px;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 10px;
  }

  .operation {
    padding: 15px;
    border: 1px solid #292e35;
    border-radius: 9px;
    background: #14171b;
  }

  .operation-topline {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .mode {
    padding: 3px 6px;
    border: 1px solid #323740;
    border-radius: 5px;
    color: #868d97;
    font-size: 10px;
    text-transform: uppercase;
  }

  .mode.write {
    border-color: #493f2d;
    color: #bda777;
  }

  .availability {
    display: inline-block;
    margin-top: 11px;
    font-size: 11px;
    font-weight: 650;
  }

  .availability[data-availability="available"] {
    color: #8db79b;
  }

  .availability[data-availability="parameterized"],
  .availability[data-availability="contextRequired"] {
    color: #c1a574;
  }

  .availability[data-availability="unavailable"] {
    color: #9b7474;
  }

  .operation p {
    margin: 8px 0 0;
    font-size: 12px;
  }

  .parameters {
    margin-top: 10px;
    display: flex;
    flex-wrap: wrap;
    gap: 5px;
  }

  .parameters code {
    padding: 3px 5px;
    border-radius: 4px;
    background: #0f1216;
    color: #89909a;
    font-size: 10px;
  }

  @media (max-width: 900px) {
    .summary-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 700px) {
    .heading,
    .operations-heading,
    .preview-controls {
      display: grid;
    }

    .preview-actions {
      justify-content: start;
      flex-wrap: wrap;
    }

    .summary-grid,
    .operations {
      grid-template-columns: 1fr;
    }
  }
</style>
