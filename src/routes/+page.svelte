<script lang="ts">
  import { onMount } from "svelte";
  import AssetInspector from "$lib/components/AssetInspector.svelte";
  import SceneBrowser from "$lib/components/SceneBrowser.svelte";
  import SceneBrowserBenchmark from "$lib/components/SceneBrowserBenchmark.svelte";
  import WorkspaceExport from "$lib/components/WorkspaceExport.svelte";
  import {
    chooseWorkspaceDirectory,
    discoverFiveMLegacy,
    discoverGtaLegacy,
    getDebugThreeViewportBenchmarkSpec,
    getDiagnosticsSummary,
    getStudioInfo,
    openDedicatedNativeWorld,
    prepareGtaRpfIndex,
    prepareGtaRpfKeys,
    recordFrontendDiagnostic,
    type DebugThreeViewportBenchmarkSpec,
    type DiagnosticsSummary,
    type FiveMDiscoveryReport,
    type GtaVDiscoveryReport,
    type StudioInfo,
  } from "$lib/native";

  type StudioView = "home" | "scene" | "assets" | "export";

  let studioInfo: StudioInfo | null = null;
  let gtaDiscovery: GtaVDiscoveryReport | null = null;
  let fivemDiscovery: FiveMDiscoveryReport | null = null;
  let gtaLegacyRoot: string | null = null;
  let workspacePath: string | null = null;
  let debugThreeBenchmark: DebugThreeViewportBenchmarkSpec | null = null;
  let activeView: StudioView = "home";
  let statusMessage = "";
  let environmentLoading = false;
  let gtaMapOpening = false;
  let diagnostics: DiagnosticsSummary | null = null;

  onMount(async () => {
    try {
      [studioInfo, debugThreeBenchmark] = await Promise.all([
        getStudioInfo(),
        getDebugThreeViewportBenchmarkSpec(),
      ]);

      if (debugThreeBenchmark) {
        workspacePath = debugThreeBenchmark.workspace;
        activeView = "scene";
      }

      await discoverEnvironment();
    } catch {
      statusMessage = "RageLab could not connect to its native desktop bridge.";
    }
  });

  onMount(() => {
    const captureError = (event: ErrorEvent) => {
      const stack =
        event.error instanceof Error
          ? event.error.stack
          : undefined;
      void recordFrontendDiagnostic(
        event.message || "Unhandled frontend error",
        stack,
        "window.error",
      );
    };

    const captureRejection = (event: PromiseRejectionEvent) => {
      const reason = event.reason;
      const message =
        reason instanceof Error
          ? reason.message
          : typeof reason === "string"
            ? reason
            : JSON.stringify(reason ?? null);
      const stack = reason instanceof Error ? reason.stack : undefined;
      void recordFrontendDiagnostic(
        message || "Unhandled promise rejection",
        stack,
        "window.unhandledrejection",
      );
    };

    window.addEventListener("error", captureError);
    window.addEventListener("unhandledrejection", captureRejection);
    void getDiagnosticsSummary().then((summary) => {
      diagnostics = summary;
    });

    return () => {
      window.removeEventListener("error", captureError);
      window.removeEventListener("unhandledrejection", captureRejection);
    };
  });

  $: gtaLegacyRoot =
    debugThreeBenchmark?.gtaLegacyRoot ??
    gtaDiscovery?.candidates.find(
      (candidate) => candidate.valid && candidate.edition === "legacy",
    )?.root ??
    null;

  $: if (!workspacePath && (activeView === "scene" || activeView === "export")) {
    activeView = "home";
  }

  async function openWorkspace() {
    statusMessage = "";

    try {
      const selected = await chooseWorkspaceDirectory();
      if (!selected) return;
      workspacePath = selected;
      activeView = "scene";
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
          : "Unable to detect GTA V Legacy and FiveM.";
    } finally {
      environmentLoading = false;
    }
  }

  async function openGtaMapDirect() {
    if (gtaMapOpening) return;

    statusMessage = "";
    gtaMapOpening = true;

    try {
      let root =
        gtaLegacyRoot ??
        gtaDiscovery?.candidates.find(
          (candidate) => candidate.valid && candidate.edition === "legacy",
        )?.root ??
        null;

      if (!root) {
        await discoverEnvironment();
        root =
          gtaDiscovery?.candidates.find(
            (candidate) => candidate.valid && candidate.edition === "legacy",
          )?.root ?? null;
      }

      if (!root) {
        throw new Error("GTA V Legacy was not detected.");
      }

      statusMessage = "Preparing GTA map…";
      const keyReport = await prepareGtaRpfKeys(root);
      const indexReport = await prepareGtaRpfIndex(root, keyReport.cache);

      await openDedicatedNativeWorld({
        gameIndex: {
          gameRoot: root,
          index: indexReport.index,
          keys: keyReport.cache,
        },
        position: [-1625, -850, 10],
        loadRadius: 1150,
        retainRadius: 1500,
        maxActiveMaps: 20,
        maxChunkLoadsPerUpdate: 2,
        maxCpuChunks: 48,
      });

      statusMessage = "GTA map opened in the dedicated RageLab Map window.";
    } catch (error) {
      statusMessage =
        error instanceof Error ? error.message : "Unable to open GTA map.";
    } finally {
      gtaMapOpening = false;
    }
  }

  function goTo(view: StudioView) {
    if ((view === "scene" || view === "export") && !workspacePath) {
      statusMessage = "Choose a workspace first. RageLab never edits your GTA installation directly.";
      activeView = "home";
      return;
    }

    statusMessage = "";
    activeView = view;
  }
</script>

<svelte:head>
  <title>RageLab Studio</title>
  <meta
    name="description"
    content="Desktop authoring interface for GTA V Legacy and FiveM resources through RageLab Core."
  />
</svelte:head>

<main class="shell">
  <aside class="sidebar">
    <button class="brand" type="button" onclick={() => goTo("home")} aria-label="RageLab home">
      <span class="mark">RL</span>
      <span class="brand-copy">
        <strong>RageLab</strong>
        <small>Studio</small>
      </span>
    </button>

    <nav aria-label="Primary">
      <button class:active={activeView === "home"} type="button" onclick={() => goTo("home")}>
        <span>Home</span>
        <small>Setup & status</small>
      </button>
      <button
        class:active={activeView === "scene"}
        type="button"
        onclick={() => goTo("scene")}
        disabled={!workspacePath}
      >
        <span>Scene</span>
        <small>World & YMAP</small>
      </button>
      <button class:active={activeView === "assets"} type="button" onclick={() => goTo("assets")}>
        <span>Assets</span>
        <small>Inspect a file</small>
      </button>
      <button
        class:active={activeView === "export"}
        type="button"
        onclick={() => goTo("export")}
        disabled={!workspacePath}
      >
        <span>Export</span>
        <small>Build resource copy</small>
      </button>
    </nav>

    <button
      class="gta-map-button"
      type="button"
      onclick={() => void openGtaMapDirect()}
      disabled={gtaMapOpening}
    >
      <span>{gtaMapOpening ? "Opening GTA Map…" : "Open GTA Map"}</span>
      <small>{gtaLegacyRoot ? "Dedicated native window" : "Detect GTA Legacy and open"}</small>
    </button>

    <div class="sidebar-context">
      <span class="context-label">GTA Legacy</span>
      <strong class:ready={Boolean(gtaLegacyRoot)}>
        {environmentLoading ? "Detecting…" : gtaLegacyRoot ? "Detected" : "Not detected"}
      </strong>
      {#if workspacePath}
        <span class="context-label">Workspace</span>
        <code>{workspacePath}</code>
      {/if}
      {#if diagnostics}
        <span class="context-label">Crash recorder</span>
        <strong class="ready">Active</strong>
        <code>{diagnostics.directory}</code>
        {#if diagnostics.latestIncident}
          <code>latest: {diagnostics.latestIncident}</code>
        {/if}
      {/if}
    </div>

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
    {#if activeView === "home"}
      <section class="home">
        {#if !workspacePath}
          <header class="hero">
            <p class="eyebrow">Start here</p>
            <h1>Open a workspace. RageLab handles the GTA data for you.</h1>
            <p class="lede">
              GTA V Legacy is used as a <strong>read-only source</strong>. Your workspace is
              where local YMAPs, assets and edits live.
            </p>
            <div class="hero-actions">
              <button class="primary large" type="button" onclick={openWorkspace}>
                Choose workspace folder
              </button>
              <button class="secondary" type="button" onclick={discoverEnvironment} disabled={environmentLoading}>
                {environmentLoading ? "Scanning…" : "Rescan GTA / FiveM"}
              </button>
            </div>
          </header>

          <div class="steps" aria-label="Getting started">
            <article class:complete={Boolean(gtaLegacyRoot)}>
              <span class="step-number">1</span>
              <div>
                <strong>GTA V Legacy</strong>
                <p>
                  {#if environmentLoading}
                    Looking for a local installation…
                  {:else if gtaLegacyRoot}
                    Found automatically. RageLab will only read from it.
                  {:else}
                    Not found. You can still inspect standalone assets.
                  {/if}
                </p>
                {#if gtaLegacyRoot}<code>{gtaLegacyRoot}</code>{/if}
              </div>
              <span class="step-state">{gtaLegacyRoot ? "Ready" : environmentLoading ? "…" : "Optional"}</span>
            </article>

            <article class="current">
              <span class="step-number">2</span>
              <div>
                <strong>Choose your workspace</strong>
                <p>Select the FiveM resource or local project folder you actually want to edit.</p>
              </div>
              <button class="inline-action" type="button" onclick={openWorkspace}>Choose folder</button>
            </article>

            <article>
              <span class="step-number">3</span>
              <div>
                <strong>Open a scene</strong>
                <p>After the workspace opens, choose a YMAP or browse the GTA world.</p>
              </div>
              <span class="step-state">Next</span>
            </article>
          </div>

          <div class="safety-note">
            <strong>Safe by default</strong>
            <span>RageLab does not overwrite your GTA installation. Game data is a read-only provider; exports are explicit copies.</span>
          </div>
        {:else}
          <header class="workspace-hero">
            <div>
              <p class="eyebrow">Workspace ready</p>
              <h1>What do you want to do?</h1>
              <p class="lede">Pick a task. Advanced RPF and renderer controls stay out of the way until you need them.</p>
            </div>
            <button class="secondary" type="button" onclick={openWorkspace}>Change workspace</button>
          </header>

          <div class="workspace-card">
            <span>Current workspace</span>
            <code>{workspacePath}</code>
            <div class="provider-row">
              <span class:ready={Boolean(gtaLegacyRoot)}>
                {gtaLegacyRoot ? "GTA Legacy linked · read-only" : "No GTA Legacy provider"}
              </span>
              {#if fivemDiscovery?.validInstallations}
                <span>{fivemDiscovery.validInstallations} FiveM install{fivemDiscovery.validInstallations === 1 ? "" : "s"} detected</span>
              {/if}
            </div>
          </div>

          <div class="task-grid">
            <button class="task-card primary-task" type="button" onclick={() => goTo("scene")}>
              <span class="task-kicker">Most common</span>
              <strong>Open a YMAP scene</strong>
              <p>Load your map, see real GTA geometry, select entities and edit the workspace copy.</p>
              <span class="task-cta">Open Scene →</span>
            </button>

            <button class="task-card" type="button" onclick={() => goTo("scene")}>
              <span class="task-kicker">World</span>
              <strong>Browse GTA V Legacy</strong>
              <p>Search the indexed world and inspect nearby YMAPs and resolved game assets.</p>
              <span class="task-cta">{gtaLegacyRoot ? "Open World →" : "Requires GTA Legacy"}</span>
            </button>

            <button class="task-card" type="button" onclick={() => goTo("assets")}>
              <span class="task-kicker">Single file</span>
              <strong>Inspect an asset</strong>
              <p>Open a YDR, YDD, YFT, YTD or supported metadata file without loading a scene.</p>
              <span class="task-cta">Inspect Asset →</span>
            </button>

            <button class="task-card" type="button" onclick={() => goTo("export")}>
              <span class="task-kicker">Output</span>
              <strong>Export a resource</strong>
              <p>Validate selected workspace maps and create a new FiveM resource copy.</p>
              <span class="task-cta">Export →</span>
            </button>
          </div>
        {/if}

        {#if statusMessage}
          <div class="status" role="status">{statusMessage}</div>
        {/if}
      </section>
    {:else if activeView === "scene"}
      <header class="tool-header">
        <div>
          <p class="eyebrow">Maps</p>
          <h1>Launch a dedicated map window</h1>
          <p>The Studio shell only chooses/configures what to open. YMAP scenes and the GTA world render in the separate native <strong>RageLab Map</strong> window.</p>
        </div>
        <button class="secondary" type="button" onclick={() => goTo("home")}>Back to home</button>
      </header>
      {#if debugThreeBenchmark}
        <SceneBrowserBenchmark {workspacePath} {gtaLegacyRoot} debugThreeBenchmark={debugThreeBenchmark} />
      {:else}
        <SceneBrowser {workspacePath} {gtaLegacyRoot} />
      {/if}
    {:else if activeView === "assets"}
      <header class="tool-header">
        <div>
          <p class="eyebrow">Assets</p>
          <h1>Inspect one asset</h1>
          <p>Use this when you already have a specific RAGE file and do not need a full YMAP scene.</p>
        </div>
        <button class="secondary" type="button" onclick={() => goTo("home")}>Back to home</button>
      </header>
      <AssetInspector />
    {:else if activeView === "export"}
      <header class="tool-header">
        <div>
          <p class="eyebrow">Export</p>
          <h1>Build a validated resource copy</h1>
          <p>Export never overwrites the workspace. Choose the maps to include, validate them, then write to a new output folder.</p>
        </div>
        <button class="secondary" type="button" onclick={() => goTo("home")}>Back to home</button>
      </header>
      <WorkspaceExport {workspacePath} />
    {/if}
  </section>
</main>

<style>
  :global(*) { box-sizing: border-box; }

  :global(html) {
    background: #0c0e11;
    color-scheme: dark;
    font-family: Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI", sans-serif;
  }

  :global(body) {
    margin: 0;
    min-width: 320px;
    min-height: 100vh;
    background: #0c0e11;
    color: #f3f4f6;
  }

  button { font: inherit; }

  .shell {
    min-height: 100vh;
    display: grid;
    grid-template-columns: 216px minmax(0, 1fr);
  }

  .sidebar {
    position: sticky;
    top: 0;
    height: 100vh;
    padding: 18px 14px;
    border-right: 1px solid #23272e;
    background: #101216;
    display: flex;
    flex-direction: column;
    gap: 24px;
  }

  .brand {
    width: 100%;
    padding: 4px;
    border: 0;
    background: transparent;
    color: inherit;
    display: flex;
    align-items: center;
    gap: 11px;
    text-align: left;
    cursor: pointer;
  }

  .mark {
    width: 34px;
    height: 34px;
    border: 1px solid #3a4049;
    border-radius: 9px;
    display: grid;
    place-items: center;
    background: #181b20;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: .08em;
  }

  .brand-copy { display: grid; gap: 1px; }
  .brand-copy strong { font-size: 14px; }
  .brand-copy small { color: #777e89; font-size: 11px; }

  nav { display: grid; gap: 5px; }

  nav button {
    width: 100%;
    padding: 10px 11px;
    border: 1px solid transparent;
    border-radius: 8px;
    background: transparent;
    color: #8a919c;
    display: grid;
    gap: 2px;
    text-align: left;
    cursor: pointer;
  }

  nav button span { font-size: 13px; font-weight: 650; }
  nav button small { color: #606873; font-size: 10px; }
  nav button:hover:not(:disabled) { background: #171a1f; color: #e5e7eb; }
  nav button.active { border-color: #303640; background: #1a1e24; color: #fff; }
  nav button.active small { color: #929aa5; }
  nav button:disabled { opacity: .35; cursor: default; }

  .gta-map-button {
    width: 100%;
    padding: 12px;
    border: 1px solid #e5e7eb;
    border-radius: 8px;
    background: #f3f4f6;
    color: #111318;
    display: grid;
    gap: 2px;
    text-align: left;
    cursor: pointer;
    font-weight: 750;
  }

  .gta-map-button span {
    font-size: 12px;
  }

  .gta-map-button small {
    color: #555d68;
    font-size: 9px;
    font-weight: 600;
  }

  .gta-map-button:hover:not(:disabled) {
    background: #fff;
  }

  .gta-map-button:disabled {
    opacity: .55;
    cursor: default;
  }

  .sidebar-context {
    padding: 12px;
    border: 1px solid #272c33;
    border-radius: 9px;
    background: #0d0f13;
    display: grid;
    gap: 5px;
  }

  .sidebar-context .context-label {
    margin-top: 5px;
    color: #666e79;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: .06em;
    text-transform: uppercase;
  }

  .sidebar-context strong { color: #a1a8b2; font-size: 11px; }
  .sidebar-context strong.ready, .provider-row .ready { color: #8fc6a4; }
  .sidebar-context code { color: #858d98; font-size: 9px; overflow-wrap: anywhere; }

  .version {
    margin-top: auto;
    padding: 0 8px;
    color: #5d646e;
    font-size: 10px;
    display: grid;
    gap: 2px;
  }

  .content {
    width: min(1260px, calc(100vw - 216px));
    padding: 46px 54px 70px;
  }

  .home { max-width: 1020px; }

  .hero, .workspace-hero, .tool-header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 32px;
  }

  .hero { display: block; max-width: 840px; }
  .workspace-hero { margin-bottom: 22px; }

  .eyebrow {
    margin: 0 0 10px;
    color: #79818c;
    font-size: 11px;
    font-weight: 750;
    letter-spacing: .08em;
    text-transform: uppercase;
  }

  h1 {
    margin: 0;
    max-width: 800px;
    font-size: clamp(30px, 4vw, 46px);
    line-height: 1.05;
    letter-spacing: -.035em;
  }

  .tool-header h1 { font-size: 31px; }

  .lede, .tool-header p:not(.eyebrow), .workspace-hero p:not(.eyebrow) {
    max-width: 720px;
    margin: 15px 0 0;
    color: #9aa1ab;
    font-size: 14px;
    line-height: 1.6;
  }

  .hero-actions {
    margin-top: 28px;
    display: flex;
    gap: 9px;
    flex-wrap: wrap;
  }

  .primary, .secondary, .inline-action {
    border-radius: 7px;
    padding: 9px 13px;
    font-weight: 650;
    cursor: pointer;
  }

  .primary {
    border: 1px solid #e5e7eb;
    background: #f3f4f6;
    color: #111318;
  }

  .primary.large { padding: 11px 16px; }
  .primary:hover { background: #fff; }

  .secondary, .inline-action {
    border: 1px solid #343a43;
    background: #171a1f;
    color: #d9dde3;
  }

  .secondary:hover:not(:disabled), .inline-action:hover { border-color: #4b535f; background: #1d2127; }
  .secondary:disabled { opacity: .5; cursor: default; }

  .steps {
    margin-top: 42px;
    display: grid;
    gap: 10px;
  }

  .steps article {
    min-height: 96px;
    padding: 18px;
    border: 1px solid #272c33;
    border-radius: 11px;
    background: #111419;
    display: grid;
    grid-template-columns: 34px minmax(0, 1fr) auto;
    gap: 14px;
    align-items: center;
  }

  .steps article.current { border-color: #49515d; background: #15191e; }
  .steps article.complete .step-number { border-color: #355744; color: #9dceb0; }

  .step-number {
    width: 30px;
    height: 30px;
    border: 1px solid #363c45;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #aab0b9;
    font-size: 11px;
    font-weight: 800;
  }

  .steps strong { font-size: 14px; }
  .steps p { margin: 5px 0 0; color: #858d98; font-size: 12px; line-height: 1.45; }
  .steps code { display: block; margin-top: 6px; color: #717986; font-size: 9px; overflow-wrap: anywhere; }
  .step-state { color: #7d8590; font-size: 10px; font-weight: 700; text-transform: uppercase; }

  .safety-note {
    margin-top: 16px;
    padding: 14px 16px;
    border: 1px solid #25362d;
    border-radius: 9px;
    background: #101712;
    display: flex;
    gap: 12px;
    align-items: baseline;
    color: #819087;
    font-size: 11px;
  }

  .safety-note strong { color: #a7c3b0; white-space: nowrap; }

  .workspace-card {
    padding: 16px 18px;
    border: 1px solid #2b3038;
    border-radius: 10px;
    background: #111419;
    display: grid;
    gap: 7px;
  }

  .workspace-card > span {
    color: #6e7681;
    font-size: 9px;
    font-weight: 750;
    letter-spacing: .06em;
    text-transform: uppercase;
  }

  .workspace-card code {
    color: #c4c9d0;
    font-size: 11px;
    overflow-wrap: anywhere;
  }

  .provider-row {
    margin-top: 4px;
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }

  .provider-row span {
    padding: 3px 7px;
    border: 1px solid #303640;
    border-radius: 20px;
    color: #838b96;
    font-size: 9px;
  }

  .task-grid {
    margin-top: 18px;
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
  }

  .task-card {
    min-height: 176px;
    padding: 20px;
    border: 1px solid #292f37;
    border-radius: 11px;
    background: #111419;
    color: inherit;
    display: grid;
    align-content: start;
    gap: 8px;
    text-align: left;
    cursor: pointer;
  }

  .task-card:hover { border-color: #454e5a; background: #15191f; }
  .task-card.primary-task { border-color: #3a424e; background: #161a20; }
  .task-kicker { color: #717985; font-size: 9px; font-weight: 750; letter-spacing: .07em; text-transform: uppercase; }
  .task-card strong { font-size: 17px; letter-spacing: -.015em; }
  .task-card p { margin: 0; color: #858d98; font-size: 11px; line-height: 1.55; }
  .task-cta { margin-top: 8px; color: #c5cad1; font-size: 11px; font-weight: 700; }

  .tool-header {
    margin-bottom: 20px;
    padding-bottom: 20px;
    border-bottom: 1px solid #252a31;
  }

  .status {
    margin-top: 16px;
    padding: 10px 12px;
    border: 1px solid #4a3d2c;
    border-radius: 7px;
    background: #17130f;
    color: #c6a778;
    font-size: 11px;
  }

  @media (max-width: 820px) {
    .shell { grid-template-columns: 1fr; }
    .sidebar { position: static; width: 100%; height: auto; border-right: 0; border-bottom: 1px solid #23272e; }
    nav { grid-template-columns: repeat(4, minmax(0, 1fr)); }
    nav button small, .sidebar-context, .version { display: none; }
    .content { width: 100%; padding: 30px 20px 50px; }
    .workspace-hero, .tool-header { display: grid; }
    .task-grid { grid-template-columns: 1fr; }
  }

  @media (max-width: 560px) {
    nav { grid-template-columns: repeat(2, minmax(0, 1fr)); }
    .steps article { grid-template-columns: 30px minmax(0, 1fr); }
    .steps .step-state, .steps .inline-action { grid-column: 2; justify-self: start; }
    .safety-note { display: grid; }
  }
</style>
