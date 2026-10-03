<script lang="ts">
  import {
    buildExportOutputPath,
    chooseExportParentDirectory,
    chooseWorkspaceYmaps,
    exportWorkspace,
    preflightWorkspaceExport,
    type WorkspaceExportPreflightReport,
    type WorkspaceExportReport,
  } from "$lib/native";

  export let workspacePath: string | null = null;

  let activeWorkspacePath: string | null = workspacePath;
  let selectedMaps: string[] = [];
  let outputParent: string | null = null;
  let resourceName = "";
  let outputPath = "";
  let outputPathError = "";
  let allowUnresolved = false;
  let preflight: WorkspaceExportPreflightReport | null = null;
  let exportReport: WorkspaceExportReport | null = null;
  let preflightLoading = false;
  let exportLoading = false;
  let errorMessage = "";
  let outputPathGeneration = 0;
  let requestGeneration = 0;
  let canPreflight = false;
  let exportGateSatisfied = false;
  let canExport = false;

  $: if (workspacePath !== activeWorkspacePath) {
    activeWorkspacePath = workspacePath;
    selectedMaps = [];
    outputParent = null;
    resourceName = "";
    outputPath = "";
    outputPathError = "";
    allowUnresolved = false;
    preflight = null;
    exportReport = null;
    errorMessage = "";
    preflightLoading = false;
    exportLoading = false;
    requestGeneration += 1;
    outputPathGeneration += 1;
  }

  $: void refreshOutputPath(outputParent, resourceName);

  $: canPreflight =
    Boolean(workspacePath) &&
    selectedMaps.length > 0 &&
    !preflightLoading &&
    !exportLoading;

  $: exportGateSatisfied =
    preflight !== null &&
    (preflight.exportGate.allowedWithoutOverride ||
      (preflight.exportGate.requiresAllowUnresolved && allowUnresolved));

  $: canExport =
    Boolean(workspacePath) &&
    selectedMaps.length > 0 &&
    preflight !== null &&
    exportGateSatisfied &&
    Boolean(outputParent) &&
    Boolean(resourceName.trim()) &&
    Boolean(outputPath) &&
    !outputPathError &&
    !preflightLoading &&
    !exportLoading &&
    exportReport === null;

  async function refreshOutputPath(parent: string | null, name: string) {
    const generation = ++outputPathGeneration;
    outputPathError = "";

    const trimmedName = name.trim();
    if (!parent || !trimmedName) {
      outputPath = "";
      return;
    }

    try {
      const joined = await buildExportOutputPath(parent, trimmedName);
      if (generation === outputPathGeneration) {
        outputPath = joined;
      }
    } catch (error) {
      if (generation === outputPathGeneration) {
        outputPath = "";
        outputPathError = errorMessageFor(
          error,
          "Unable to resolve the export output path.",
        );
      }
    }
  }

  async function chooseMaps() {
    if (!workspacePath) {
      errorMessage = "Open a workspace before selecting export YMAPs.";
      return;
    }

    errorMessage = "";

    try {
      const selected = await chooseWorkspaceYmaps(workspacePath);
      if (selected.length === 0) {
        return;
      }

      selectedMaps = [...new Set(selected)];
      invalidatePreflight();
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "Unable to choose workspace YMAPs.",
      );
    }
  }

  function removeMap(path: string) {
    selectedMaps = selectedMaps.filter((candidate) => candidate !== path);
    invalidatePreflight();
  }

  function invalidatePreflight() {
    requestGeneration += 1;
    preflight = null;
    exportReport = null;
    allowUnresolved = false;
    errorMessage = "";
    preflightLoading = false;
    exportLoading = false;
  }

  async function chooseOutputParent() {
    errorMessage = "";

    try {
      const selected = await chooseExportParentDirectory();
      if (!selected) {
        return;
      }

      outputParent = selected;
      exportReport = null;
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "Unable to choose the export parent directory.",
      );
    }
  }

  function updateResourceName(value: string) {
    resourceName = value;
    exportReport = null;
    errorMessage = "";
  }

  function updateAllowUnresolved(value: boolean) {
    allowUnresolved = value;
    exportReport = null;
    errorMessage = "";
  }

  async function runPreflight() {
    if (!workspacePath || selectedMaps.length === 0) {
      errorMessage = "Select at least one workspace YMAP before preflight.";
      return;
    }

    const generation = ++requestGeneration;
    preflightLoading = true;
    exportLoading = false;
    errorMessage = "";
    preflight = null;
    exportReport = null;
    allowUnresolved = false;

    try {
      const report = await preflightWorkspaceExport(workspacePath, selectedMaps);
      if (generation === requestGeneration) {
        preflight = report;
      }
    } catch (error) {
      if (generation === requestGeneration) {
        errorMessage = errorMessageFor(
          error,
          "RageLab core could not preflight this workspace export.",
        );
      }
    } finally {
      if (generation === requestGeneration) {
        preflightLoading = false;
      }
    }
  }

  async function runExport() {
    if (
      !workspacePath ||
      !preflight ||
      !exportGateSatisfied ||
      !outputParent ||
      !resourceName.trim()
    ) {
      errorMessage =
        "Complete a successful preflight and explicit output configuration before export.";
      return;
    }

    const generation = ++requestGeneration;
    exportLoading = true;
    errorMessage = "";
    exportReport = null;

    try {
      const destination = await buildExportOutputPath(
        outputParent,
        resourceName.trim(),
      );

      if (generation !== requestGeneration) {
        return;
      }

      outputPath = destination;
      const report = await exportWorkspace({
        workspace: workspacePath,
        maps: selectedMaps,
        output: destination,
        resourceName: resourceName.trim(),
        allowUnresolved,
      });

      if (generation === requestGeneration) {
        exportReport = report;
      }
    } catch (error) {
      if (generation === requestGeneration) {
        exportReport = null;
        errorMessage = errorMessageFor(
          error,
          "RageLab core could not create this workspace export.",
        );
      }
    } finally {
      if (generation === requestGeneration) {
        exportLoading = false;
      }
    }
  }

  function fileName(path: string): string {
    return path.split(/[\\/]/).filter(Boolean).at(-1) ?? path;
  }

  function errorMessageFor(error: unknown, fallback: string): string {
    if (typeof error === "string" && error.trim()) {
      return error;
    }
    return error instanceof Error && error.message ? error.message : fallback;
  }
</script>

<section class="export-workflow" id="workspace-export" aria-live="polite">
  <div class="heading">
    <div>
      <p class="label">Workspace export</p>
      <h2>Legacy resource export</h2>
      <p class="detail">
        Select one or more workspace YMAP roots, run the Rust preflight, then
        create a new resource directory outside the source workspace.
      </p>
    </div>

    <div class="heading-actions">
      <button
        class="secondary"
        type="button"
        onclick={chooseMaps}
        disabled={!workspacePath || preflightLoading || exportLoading}
      >
        Select YMAPs
      </button>
      <button
        class="secondary"
        type="button"
        onclick={chooseOutputParent}
        disabled={!workspacePath || exportLoading}
      >
        Choose output
      </button>
    </div>
  </div>

  {#if !workspacePath}
    <div class="empty-state">
      <p class="empty-title">Open a workspace to configure export</p>
      <p class="detail">
        Dependency closure, unresolved classification, export gating, writes,
        and validation remain owned by the pinned RageLab core.
      </p>
    </div>
  {:else}
    <div class="configuration-grid">
      <div class="config-card">
        <div class="config-heading">
          <div>
            <span class="step">01</span>
            <strong>Root YMAPs</strong>
          </div>
          <span class="count">{selectedMaps.length} selected</span>
        </div>

        {#if selectedMaps.length > 0}
          <div class="path-list">
            {#each selectedMaps as map}
              <div class="path-row">
                <div>
                  <span class="path-name">{fileName(map)}</span>
                  <code>{map}</code>
                </div>
                <button
                  class="text-button"
                  type="button"
                  onclick={() => removeMap(map)}
                  disabled={preflightLoading || exportLoading}
                >
                  Remove
                </button>
              </div>
            {/each}
          </div>
        {:else}
          <p class="muted">
            No roots selected. The core will reject export until at least one
            YMAP inside the workspace is selected.
          </p>
        {/if}

        <button
          class="primary"
          type="button"
          onclick={runPreflight}
          disabled={!canPreflight}
        >
          {preflightLoading ? "Running preflight…" : "Run preflight"}
        </button>
      </div>

      <div class="config-card">
        <div class="config-heading">
          <div>
            <span class="step">02</span>
            <strong>Output</strong>
          </div>
          <span class="count">create-new only</span>
        </div>

        <label class="field">
          <span>Resource name</span>
          <input
            type="text"
            value={resourceName}
            oninput={(event) => updateResourceName(event.currentTarget.value)}
            placeholder="my_resource"
            disabled={exportLoading}
          />
        </label>

        <div class="path-block">
          <span>Parent directory</span>
          {#if outputParent}
            <code>{outputParent}</code>
          {:else}
            <em>Choose a directory outside the source workspace.</em>
          {/if}
        </div>

        <div class="path-block">
          <span>New resource path</span>
          {#if outputPath}
            <code>{outputPath}</code>
          {:else}
            <em>Set a parent directory and resource name.</em>
          {/if}
        </div>

        {#if outputPathError}
          <p class="inline-error">{outputPathError}</p>
        {/if}
      </div>
    </div>

    <div class="report-card">
      <div class="report-heading">
        <div>
          <span class="step">03</span>
          <div>
            <strong>Preflight</strong>
            <span>Core-owned dependency closure and export gate</span>
          </div>
        </div>

        {#if preflight}
          <span
            class:gate-ok={preflight.exportGate.allowedWithoutOverride}
            class:gate-warn={preflight.exportGate.requiresAllowUnresolved}
            class="gate"
          >
            {preflight.exportGate.allowedWithoutOverride
              ? "ready"
              : "override required"}
          </span>
        {/if}
      </div>

      {#if preflight}
        <div class="metrics">
          <div>
            <strong>{preflight.selectedRoots.length}</strong>
            <span>selected roots</span>
          </div>
          <div>
            <strong>{preflight.closureYmaps.length}</strong>
            <span>closure YMAPs</span>
          </div>
          <div>
            <strong>{preflight.predictedFiles.length}</strong>
            <span>predicted files</span>
          </div>
          <div>
            <strong>{preflight.unresolved.unknown}</strong>
            <span>unknown refs</span>
          </div>
        </div>

        <div class="unresolved-grid">
          <span>raw <strong>{preflight.unresolved.raw}</strong></span>
          <span>vanilla <strong>{preflight.unresolved.vanilla}</strong></span>
          <span>unknown <strong>{preflight.unresolved.unknown}</strong></span>
        </div>

        {#if preflight.exportGate.requiresAllowUnresolved}
          <label class="override">
            <input
              type="checkbox"
              checked={allowUnresolved}
              onchange={(event) =>
                updateAllowUnresolved(event.currentTarget.checked)}
              disabled={exportLoading}
            />
            <span>
              <strong>Allow unresolved dependencies for this export</strong>
              <small>
                This explicit override is required by the core because the
                preflight contains {preflight.unresolved.unknown} unknown
                reference(s).
              </small>
            </span>
          </label>
        {/if}

        {#if preflight.warnings.length > 0}
          <div class="message-list warning-list">
            <strong>Warnings</strong>
            <ul>
              {#each preflight.warnings as warning}
                <li>{warning}</li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if preflight.unknownGroups.length > 0}
          <details>
            <summary>
              Unknown dependency groups ({preflight.unknownGroups.length})
            </summary>
            <div class="detail-list">
              {#each preflight.unknownGroups as group}
                <article>
                  <div class="detail-title">
                    <code>{group.kind} {group.hash}</code>
                    <span>{group.uses} use(s)</span>
                  </div>
                  {#if group.reasons.length > 0}
                    <p>{group.reasons.join(" · ")}</p>
                  {/if}
                  {#if group.affectedMaps.length > 0}
                    <small>{group.affectedMaps.join(", ")}</small>
                  {/if}
                </article>
              {/each}
            </div>
          </details>
        {/if}

        {#if preflight.mloAudits.length > 0}
          <details>
            <summary>MLO audits ({preflight.mloAudits.length})</summary>
            <div class="detail-list">
              {#each preflight.mloAudits as audit}
                <article>
                  <div class="detail-title">
                    <code>{audit.archetypeHash}</code>
                    <span>{audit.risk}</span>
                  </div>
                  <p>
                    {audit.ytyp} · {audit.entities} entities · {audit.rooms}
                    rooms · {audit.portals} portals
                  </p>
                  <small>
                    local {audit.localFiles} · vanilla {audit.vanilla} · unknown
                    {audit.unknown}
                  </small>
                </article>
              {/each}
            </div>
          </details>
        {/if}

        <details>
          <summary>Predicted files ({preflight.predictedFiles.length})</summary>
          <div class="simple-list">
            {#each preflight.predictedFiles as path}
              <code>{path}</code>
            {/each}
          </div>
        </details>
      {:else}
        <p class="muted">
          No preflight report yet. Changing the selected YMAP roots invalidates
          the previous preflight and any export result.
        </p>
      {/if}
    </div>

    <div class="report-card">
      <div class="report-heading">
        <div>
          <span class="step">04</span>
          <div>
            <strong>Export</strong>
            <span>Non-destructive resource creation and semantic validation</span>
          </div>
        </div>

        <button
          class="primary"
          type="button"
          onclick={runExport}
          disabled={!canExport}
        >
          {exportLoading
            ? "Exporting…"
            : exportReport
              ? "Export complete"
              : "Create resource"}
        </button>
      </div>

      {#if preflight && !exportGateSatisfied}
        <p class="gate-message">
          Export remains gated until the preflight is allowed or the explicit
          unresolved override is enabled.
        </p>
      {:else if preflight && (!outputParent || !resourceName.trim())}
        <p class="muted">
          Configure an output parent and resource name to enable export.
        </p>
      {/if}

      {#if exportReport}
        <div class="result-banner" class:invalid={!exportReport.validation.valid}>
          <div>
            <strong>
              {exportReport.validation.valid
                ? "Export validated"
                : "Export validation failed"}
            </strong>
            <span>{exportReport.validation.status}</span>
          </div>
          <code>{exportReport.output.resource}</code>
        </div>

        <div class="metrics result-metrics">
          <div>
            <strong>{exportReport.output.copiedFiles.length}</strong>
            <span>copied files</span>
          </div>
          <div>
            <strong>{exportReport.output.manifestMaps}</strong>
            <span>manifest maps</span>
          </div>
          <div>
            <strong>{exportReport.validation.interiorMaps}</strong>
            <span>interior maps</span>
          </div>
          <div>
            <strong>{exportReport.unresolved.unknown}</strong>
            <span>unknown refs</span>
          </div>
        </div>

        <div class="artifact-grid">
          <div>
            <span>Stream</span>
            <code>{exportReport.output.stream}</code>
          </div>
          <div>
            <span>Manifest</span>
            <code>{exportReport.output.manifest}</code>
          </div>
          <div>
            <span>Metadata</span>
            <code>{exportReport.output.metadata}</code>
          </div>
          <div>
            <span>GTXD</span>
            {#if exportReport.output.gtxd}
              <code>{exportReport.output.gtxd}</code>
            {:else}
              <em>not emitted</em>
            {/if}
          </div>
        </div>

        {#if exportReport.output.copiedFiles.length > 0}
          <details open>
            <summary>
              Copied files ({exportReport.output.copiedFiles.length})
            </summary>
            <div class="simple-list">
              {#each exportReport.output.copiedFiles as path}
                <code>{path}</code>
              {/each}
            </div>
          </details>
        {/if}

        <p class="contract-note">
          The current Rust export report does not expose a skipped-file list.
          Studio intentionally does not derive one from predicted versus copied
          paths.
        </p>

        {#if exportReport.warnings.length > 0 ||
        exportReport.validation.warnings.length > 0}
          <div class="message-list warning-list">
            <strong>Export warnings</strong>
            <ul>
              {#each [
                ...exportReport.warnings,
                ...exportReport.validation.warnings,
              ] as warning}
                <li>{warning}</li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if exportReport.validation.errors.length > 0}
          <div class="message-list error-list">
            <strong>Validation errors</strong>
            <ul>
              {#each exportReport.validation.errors as error}
                <li>{error}</li>
              {/each}
            </ul>
          </div>
        {/if}

        {#if exportReport.validation.localMissing.length > 0}
          <details>
            <summary>
              Local missing ({exportReport.validation.localMissing.length})
            </summary>
            <div class="simple-list">
              {#each exportReport.validation.localMissing as path}
                <code>{path}</code>
              {/each}
            </div>
          </details>
        {/if}
      {:else}
        <p class="muted">
          Export creates a new output directory. Existing outputs are rejected
          by the native bridge; source workspace files are not overwritten.
        </p>
      {/if}
    </div>
  {/if}

  {#if errorMessage}
    <p class="error-message">{errorMessage}</p>
  {/if}
</section>

<style>
  .export-workflow {
    margin-top: 18px;
    padding: 24px;
    border: 1px solid #25292f;
    border-radius: 12px;
    background: rgba(17, 20, 24, 0.62);
  }

  .heading,
  .report-heading,
  .config-heading,
  .detail-title,
  .result-banner {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 18px;
  }

  .heading {
    margin-bottom: 20px;
  }

  .heading h2 {
    margin: 0;
    font-size: 20px;
    letter-spacing: -0.02em;
  }

  .label {
    margin: 0 0 8px;
    color: #7d838c;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
  }

  .detail,
  .muted {
    margin: 8px 0 0;
    color: #858b95;
    line-height: 1.55;
  }

  .heading-actions {
    display: flex;
    gap: 8px;
    flex: none;
  }

  .primary,
  .secondary {
    border-radius: 7px;
    padding: 9px 13px;
    font: inherit;
    font-weight: 650;
    cursor: pointer;
  }

  .primary {
    border: 1px solid #d9dce1;
    background: #f4f4f5;
    color: #111318;
  }

  .secondary {
    border: 1px solid #343941;
    background: #171a1f;
    color: #d9dce1;
  }

  .primary:disabled,
  .secondary:disabled {
    cursor: default;
    opacity: 0.45;
  }

  .empty-state,
  .config-card,
  .report-card {
    border: 1px solid #272b32;
    border-radius: 9px;
    background: #14171b;
  }

  .empty-state {
    padding: 18px;
  }

  .empty-title {
    margin: 0;
    font-size: 16px;
    font-weight: 650;
  }

  .configuration-grid {
    display: grid;
    grid-template-columns: minmax(0, 1.25fr) minmax(280px, 0.75fr);
    gap: 12px;
  }

  .config-card,
  .report-card {
    padding: 18px;
  }

  .report-card {
    margin-top: 12px;
  }

  .config-heading {
    align-items: center;
    margin-bottom: 16px;
  }

  .config-heading > div,
  .report-heading > div:first-child {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .step {
    display: inline-grid;
    place-items: center;
    min-width: 28px;
    height: 24px;
    padding: 0 7px;
    border: 1px solid #343941;
    border-radius: 999px;
    color: #8d949e;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }

  .count,
  .report-heading span:not(.step),
  .path-block > span,
  .field > span,
  .artifact-grid span,
  .result-banner span,
  .detail-title span {
    color: #777e88;
    font-size: 12px;
  }

  .path-list {
    display: grid;
    gap: 8px;
    margin-bottom: 14px;
  }

  .path-row {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 12px;
    padding: 10px 0;
    border-bottom: 1px solid #24282e;
  }

  .path-row:last-child {
    border-bottom: 0;
  }

  .path-name {
    display: block;
    margin-bottom: 4px;
    font-size: 12px;
    font-weight: 650;
  }

  code {
    display: block;
    overflow-wrap: anywhere;
    color: #dfe1e5;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
    font-size: 11px;
    line-height: 1.45;
  }

  .text-button {
    border: 0;
    background: transparent;
    color: #8e959f;
    font: inherit;
    font-size: 11px;
    cursor: pointer;
  }

  .text-button:hover:not(:disabled) {
    color: #f1f2f3;
  }

  .text-button:disabled {
    opacity: 0.45;
  }

  .field,
  .path-block {
    display: grid;
    gap: 7px;
    margin-bottom: 13px;
  }

  input[type="text"] {
    width: 100%;
    min-width: 0;
    border: 1px solid #333840;
    border-radius: 7px;
    padding: 9px 10px;
    background: #101216;
    color: #e6e8eb;
    font: inherit;
    font-size: 13px;
    outline: none;
  }

  input[type="text"]:focus {
    border-color: #5a616c;
  }

  .path-block em {
    color: #666d77;
    font-size: 12px;
    font-style: normal;
  }

  .report-heading {
    align-items: center;
    margin-bottom: 16px;
  }

  .report-heading > div:first-child > div {
    display: grid;
    gap: 3px;
  }

  .gate {
    border: 1px solid #3a4048;
    border-radius: 999px;
    padding: 5px 8px;
    color: #9399a2;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.05em;
    text-transform: uppercase;
  }

  .gate-ok {
    border-color: #345543;
    color: #8fc5a4;
  }

  .gate-warn {
    border-color: #654d31;
    color: #d0a56b;
  }

  .metrics {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }

  .metrics > div {
    min-height: 78px;
    padding: 12px;
    border: 1px solid #272c33;
    border-radius: 7px;
    background: #111419;
    display: grid;
    align-content: center;
    gap: 4px;
  }

  .metrics strong {
    font-size: 22px;
    letter-spacing: -0.03em;
  }

  .metrics span {
    color: #757c86;
    font-size: 11px;
  }

  .unresolved-grid {
    display: flex;
    gap: 8px;
    margin-top: 8px;
    flex-wrap: wrap;
  }

  .unresolved-grid span {
    padding: 6px 8px;
    border-radius: 6px;
    background: #101216;
    color: #858c96;
    font-size: 11px;
  }

  .unresolved-grid strong {
    color: #d7d9dd;
  }

  .override {
    margin-top: 14px;
    padding: 12px;
    border: 1px solid #5f492f;
    border-radius: 8px;
    background: rgba(86, 60, 29, 0.16);
    display: flex;
    align-items: flex-start;
    gap: 10px;
    cursor: pointer;
  }

  .override input {
    margin-top: 3px;
  }

  .override span {
    display: grid;
    gap: 3px;
  }

  .override small {
    color: #a58e70;
    line-height: 1.45;
  }

  details {
    margin-top: 12px;
    border-top: 1px solid #282c33;
    padding-top: 10px;
  }

  summary {
    color: #a5abb4;
    font-size: 12px;
    font-weight: 650;
    cursor: pointer;
  }

  .detail-list,
  .simple-list {
    display: grid;
    gap: 8px;
    margin-top: 10px;
  }

  .detail-list article {
    padding: 10px;
    border: 1px solid #272c33;
    border-radius: 7px;
    background: #101216;
  }

  .detail-list p,
  .detail-list small {
    margin: 6px 0 0;
    color: #7f8690;
    font-size: 11px;
    line-height: 1.45;
  }

  .detail-title {
    align-items: center;
  }

  .artifact-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 8px;
    margin-top: 12px;
  }

  .artifact-grid > div {
    min-width: 0;
    padding: 10px;
    border: 1px solid #272c33;
    border-radius: 7px;
    background: #101216;
    display: grid;
    gap: 5px;
  }

  .artifact-grid em {
    color: #666d77;
    font-size: 11px;
    font-style: normal;
  }

  .result-banner {
    align-items: center;
    margin-bottom: 12px;
    padding: 12px;
    border: 1px solid #345543;
    border-radius: 8px;
    background: rgba(35, 74, 51, 0.16);
  }

  .result-banner.invalid {
    border-color: #693b3b;
    background: rgba(91, 38, 38, 0.16);
  }

  .result-banner > div {
    display: grid;
    gap: 3px;
  }

  .result-banner code {
    max-width: 55%;
    text-align: right;
  }

  .result-metrics {
    margin-top: 0;
  }

  .message-list {
    margin-top: 12px;
    padding: 12px;
    border-radius: 7px;
  }

  .message-list ul {
    margin: 7px 0 0;
    padding-left: 18px;
  }

  .message-list li {
    margin: 4px 0;
    color: #a0a6af;
    font-size: 12px;
    line-height: 1.45;
  }

  .warning-list {
    border: 1px solid #5b4932;
    background: rgba(86, 60, 29, 0.12);
  }

  .error-list {
    border: 1px solid #633b3b;
    background: rgba(91, 38, 38, 0.12);
  }

  .gate-message,
  .error-message,
  .inline-error {
    color: #d0a56b;
    font-size: 12px;
    line-height: 1.45;
  }

  .gate-message,
  .inline-error {
    margin: 0;
  }

  .error-message {
    margin: 14px 0 0;
    padding: 10px 12px;
    border: 1px solid #5d4630;
    border-radius: 7px;
    background: rgba(86, 60, 29, 0.12);
  }

  .contract-note {
    margin: 12px 0 0;
    color: #6f7680;
    font-size: 11px;
    line-height: 1.5;
  }

  @media (max-width: 900px) {
    .configuration-grid,
    .artifact-grid {
      grid-template-columns: 1fr;
    }

    .metrics {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }

  @media (max-width: 620px) {
    .heading,
    .report-heading,
    .result-banner {
      display: grid;
    }

    .heading-actions {
      flex-wrap: wrap;
    }

    .metrics {
      grid-template-columns: 1fr;
    }

    .result-banner code {
      max-width: none;
      text-align: left;
    }
  }
</style>
