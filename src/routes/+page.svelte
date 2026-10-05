<script lang="ts">
  import { onMount } from "svelte";
  import AssetInspector from "$lib/components/AssetInspector.svelte";
  import SceneBrowser from "$lib/components/SceneBrowser.svelte";
  import WorkspaceExport from "$lib/components/WorkspaceExport.svelte";
  import {
    chooseWorkspaceDirectory,
    discoverFiveMLegacy,
    discoverGtaLegacy,
    getDebugThreeViewportBenchmarkSpec,
    getStudioInfo,
    type DebugThreeViewportBenchmarkSpec,
    type FiveMDiscoveryReport,
    type GtaVDiscoveryReport,
    type StudioInfo,
  } from "$lib/native";

  let studioInfo: StudioInfo | null = null;
  let gtaDiscovery: GtaVDiscoveryReport | null = null;
  let fivemDiscovery: FiveMDiscoveryReport | null = null;
  let gtaLegacyRoot: string | null = null;
  let workspacePath: string | null = null;
  let debugThreeBenchmark: DebugThreeViewportBenchmarkSpec | null = null;
  let statusMessage = "";
  let environmentLoading = false;

  onMount(async () => {
    try {
      [studioInfo, debugThreeBenchmark] = await Promise.all([
        getStudioInfo(),
        getDebugThreeViewportBenchmarkSpec(),
      ]);
      if (debugThreeBenchmark) {
        workspacePath = debugThreeBenchmark.workspace;
      }
    } catch {
      statusMessage = "Native bridge unavailable.";
    }
  });

  $: gtaLegacyRoot =
    debugThreeBenchmark?.gtaLegacyRoot ??
    gtaDiscovery?.candidates.find(
      (candidate) => candidate.valid && candidate.edition === "legacy",
    )?.root ??
    null;

  async function openWorkspace() {
    statusMessage = "";

    try {
      workspacePath = await chooseWorkspaceDirectory();
    } catch (error) {
      statusMessage =
        error instanceof Error ? error.message : "Unable to open workspace.";
    }
  }

  async function discoverEnvironment() {
    statusMessage = "";
    environmentLoading = true;

    try {
      [gtaDiscovery, fivemDiscovery] = await Promise.all([
        discoverGtaLegacy(),
        discoverFiveMLegacy(),
      ]);
    } catch (error) {
      statusMessage =
        error instanceof Error
          ? error.message
          : "Unable to detect the local RageLab environment.";
    } finally {
      environmentLoading = false;
    }
  }
</script>

<svelte:head>
  <title>RageLab Studio</title>
  <meta
    name="description"
    content="Desktop visual interface for inspecting and working with RAGE assets through RageLab."
  />
</svelte:head>

<main class="shell">
  <aside class="sidebar">
    <div class="brand">
      <span class="mark">RL</span>
      <div>
        <strong>RageLab</strong>
        <span>Studio</span>
      </div>
    </div>

    <nav aria-label="Primary">
      <button class="nav-item active" type="button">Workspace</button>
      <button
        class="nav-item"
        type="button"
        onclick={() =>
          document
            .getElementById("asset-inspector")
            ?.scrollIntoView({ behavior: "smooth", block: "start" })}
      >
        Assets
      </button>
      <button
        class="nav-item"
        type="button"
        onclick={() =>
          document
            .getElementById("scene-browser")
            ?.scrollIntoView({ behavior: "smooth", block: "start" })}
      >
        Scene
      </button>
      <button
        class="nav-item"
        type="button"
        onclick={() =>
          document
            .getElementById("workspace-export")
            ?.scrollIntoView({ behavior: "smooth", block: "start" })}
      >
        Export
      </button>
    </nav>

    <div class="version">
      {#if studioInfo}
        <span>{studioInfo.product} {studioInfo.version}</span>
        <span>core {studioInfo.coreRevision.slice(0, 7)}</span>
      {:else}
        <span>RageLab Studio</span>
      {/if}
    </div>
  </aside>

  <section class="content">
    <header>
      <div>
        <p class="eyebrow">Workspace</p>
        <h1>Open an asset workspace</h1>
        <p class="lede">
          Select a local GTA V or FiveM workspace. RageLab owns parsing,
          validation, editing, and export logic in Rust.
        </p>
      </div>

      <div class="header-actions">
        <button
          class="secondary"
          type="button"
          onclick={discoverEnvironment}
          disabled={environmentLoading}
        >
          {environmentLoading ? "Detecting…" : "Detect environment"}
        </button>

        <button class="primary" type="button" onclick={openWorkspace}>
          Open workspace
        </button>
      </div>
    </header>

    <section class="panel" aria-live="polite">
      {#if workspacePath}
        <p class="label">Selected directory</p>
        <code>{workspacePath}</code>
        <p class="detail">
          This root scopes workspace scene assembly. YMAP provider resolution,
          node transforms, collision state, and limits are evaluated by the
          pinned RageLab core.
        </p>
      {:else}
        <p class="empty-title">No workspace open</p>
        <p class="detail">
          RageLab Studio does not parse RAGE formats in the frontend. The desktop
          application delegates format and writer behavior to the Rust core.
        </p>
      {/if}

      {#if statusMessage}
        <p class="status">{statusMessage}</p>
      {/if}
    </section>

    <section class="environment" aria-live="polite">
      <div class="environment-heading">
        <div>
          <p class="label">Native environment</p>
          <h2>Legacy discovery</h2>
        </div>
        <p class="detail">
          Results come directly from the pinned RageLab Rust core.
        </p>
      </div>

      <div class="environment-grid">
        <article class="environment-card">
          <span class="environment-name">GTA V Legacy</span>
          {#if gtaDiscovery}
            <strong>{gtaDiscovery.validLegacyInstallations}</strong>
            <span class="metric-label">validated installation(s)</span>
            <span class="card-detail">
              {gtaDiscovery.candidates.length} candidate(s) inspected
            </span>
          {:else}
            <strong>—</strong>
            <span class="metric-label">not scanned</span>
          {/if}
        </article>

        <article class="environment-card">
          <span class="environment-name">FiveM</span>
          {#if fivemDiscovery}
            <strong>{fivemDiscovery.validInstallations}</strong>
            <span class="metric-label">validated installation(s)</span>
            <span class="card-detail">
              {fivemDiscovery.legacyLinkedInstallations} linked to Legacy
            </span>
          {:else}
            <strong>—</strong>
            <span class="metric-label">not scanned</span>
          {/if}
        </article>
      </div>
    </section>

    <SceneBrowser
      {workspacePath}
      {gtaLegacyRoot}
      debugThreeBenchmark={debugThreeBenchmark}
    />

    <WorkspaceExport {workspacePath} />

    <AssetInspector />
  </section>
</main>

<style>
  :global(*) {
    box-sizing: border-box;
  }

  :global(html) {
    background: #0d0f12;
    color-scheme: dark;
    font-family:
      Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont,
      "Segoe UI", sans-serif;
  }

  :global(body) {
    margin: 0;
    min-width: 320px;
    min-height: 100vh;
    background:
      radial-gradient(circle at 76% 8%, rgba(255, 255, 255, 0.035), transparent 32%),
      #0d0f12;
    color: #f4f4f5;
  }

  button {
    font: inherit;
  }

  .shell {
    min-height: 100vh;
    display: grid;
    grid-template-columns: 224px minmax(0, 1fr);
  }

  .sidebar {
    min-height: 100vh;
    padding: 22px 16px;
    border-right: 1px solid #23262b;
    background: rgba(14, 16, 19, 0.96);
    display: flex;
    flex-direction: column;
    gap: 28px;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .brand div {
    display: grid;
    gap: 1px;
  }

  .brand strong {
    font-size: 15px;
    letter-spacing: 0.01em;
  }

  .brand span:not(.mark) {
    color: #888e98;
    font-size: 12px;
  }

  .mark {
    width: 34px;
    height: 34px;
    border: 1px solid #3b4048;
    border-radius: 8px;
    display: grid;
    place-items: center;
    background: #171a1f;
    color: #d8dbe0;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.08em;
  }

  nav {
    display: grid;
    gap: 5px;
  }

  .nav-item {
    width: 100%;
    padding: 9px 10px;
    border: 0;
    border-radius: 7px;
    background: transparent;
    color: #777d87;
    text-align: left;
  }

  .nav-item.active {
    background: #1a1d22;
    color: #f5f5f5;
  }

  .nav-item:disabled {
    opacity: 0.45;
  }

  .version {
    margin-top: auto;
    padding: 0 10px;
    color: #666c75;
    font-size: 11px;
    display: grid;
    gap: 3px;
  }

  .content {
    width: min(1040px, calc(100vw - 224px));
    padding: 58px 64px;
  }

  header {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 36px;
    margin-bottom: 40px;
  }

  .eyebrow {
    margin: 0 0 10px;
    color: #787f89;
    font-size: 12px;
    font-weight: 600;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    max-width: 680px;
    font-size: clamp(32px, 4vw, 48px);
    line-height: 1.04;
    letter-spacing: -0.035em;
  }

  .lede {
    max-width: 650px;
    margin: 18px 0 0;
    color: #9aa0aa;
    line-height: 1.65;
  }

  .header-actions {
    flex: none;
    margin-top: 26px;
    display: flex;
    align-items: center;
    gap: 9px;
  }

  .primary,
  .secondary {
    border-radius: 7px;
    padding: 10px 14px;
    font-weight: 650;
    cursor: pointer;
  }

  .primary {
    border: 1px solid #d9dce1;
    background: #f4f4f5;
    color: #111318;
  }

  .primary:hover {
    background: #ffffff;
  }

  .secondary {
    border: 1px solid #343941;
    background: #171a1f;
    color: #d9dce1;
  }

  .secondary:hover:not(:disabled) {
    border-color: #4a505a;
    background: #1d2026;
  }

  .secondary:disabled {
    cursor: default;
    opacity: 0.55;
  }

  .panel {
    min-height: 180px;
    padding: 24px;
    border: 1px solid #25292f;
    border-radius: 12px;
    background: rgba(20, 23, 27, 0.7);
  }

  .label {
    margin: 0 0 10px;
    color: #7d838c;
    font-size: 12px;
    font-weight: 600;
    text-transform: uppercase;
  }

  code {
    display: block;
    overflow-wrap: anywhere;
    color: #e7e8ea;
    font-family: "SFMono-Regular", Consolas, "Liberation Mono", monospace;
    font-size: 13px;
  }

  .empty-title {
    margin: 0;
    font-size: 17px;
    font-weight: 650;
  }

  .detail {
    max-width: 720px;
    margin: 12px 0 0;
    color: #858b95;
    line-height: 1.6;
  }

  .status {
    margin: 18px 0 0;
    color: #d0a56b;
    font-size: 13px;
  }

  .environment {
    margin-top: 18px;
    padding: 24px;
    border: 1px solid #25292f;
    border-radius: 12px;
    background: rgba(17, 20, 24, 0.55);
  }

  .environment-heading {
    display: flex;
    justify-content: space-between;
    gap: 28px;
    align-items: flex-start;
    margin-bottom: 20px;
  }

  .environment-heading h2 {
    margin: 0;
    font-size: 20px;
    letter-spacing: -0.02em;
  }

  .environment-heading .detail {
    margin: 0;
    text-align: right;
  }

  .environment-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .environment-card {
    min-height: 132px;
    padding: 18px;
    border: 1px solid #272b32;
    border-radius: 9px;
    background: #14171b;
    display: grid;
    align-content: start;
    gap: 5px;
  }

  .environment-name,
  .metric-label,
  .card-detail {
    color: #7f8690;
    font-size: 12px;
  }

  .environment-card strong {
    margin-top: 8px;
    font-size: 30px;
    line-height: 1;
    letter-spacing: -0.035em;
  }

  .card-detail {
    margin-top: 8px;
  }

  @media (max-width: 760px) {
    .shell {
      grid-template-columns: 1fr;
    }

    .sidebar {
      min-height: auto;
      border-right: 0;
      border-bottom: 1px solid #23262b;
    }

    nav,
    .version {
      display: none;
    }

    .content {
      width: 100%;
      padding: 36px 24px;
    }

    header {
      flex-direction: column;
    }

    .header-actions {
      margin-top: 0;
      flex-wrap: wrap;
    }

    .environment-heading {
      display: grid;
    }

    .environment-heading .detail {
      text-align: left;
    }

    .environment-grid {
      grid-template-columns: 1fr;
    }
  }
</style>
