<script lang="ts">
  import {
    chooseTexturePngOutput,
    exportYtdTexturePng,
    type MaterialAuthoringReport,
    type MaterialFieldMode,
    type TextureAuthoringEntry,
    type TextureAuthoringReport,
  } from "$lib/native";

  export let sourcePath = "";
  export let details: unknown;

  let textureAuthoring: TextureAuthoringReport | null = null;
  let materialAuthoring: MaterialAuthoringReport | null = null;
  let exportingTexture: number | null = null;
  let exportMessage = "";
  let exportError = "";

  $: textureAuthoring = textureAuthoringReport(details);
  $: materialAuthoring = materialAuthoringReport(details);
  $: if (details) {
    exportMessage = "";
    exportError = "";
  }

  async function exportPng(texture: TextureAuthoringEntry) {
    if (!texture.export.pngTopMip || exportingTexture !== null) {
      return;
    }

    exportMessage = "";
    exportError = "";
    let output: string | null;
    try {
      output = await chooseTexturePngOutput(defaultPngPath(sourcePath, texture));
    } catch (error) {
      exportError = errorMessageFor(error, "Unable to choose PNG output.");
      return;
    }
    if (!output) {
      return;
    }

    exportingTexture = texture.index;
    try {
      const result = await exportYtdTexturePng(sourcePath, texture.index, output);
      exportMessage =
        `Exported texture #${texture.index} to ${result.output} (${formatBytes(result.bytesWritten)}; source unchanged: ${result.sourceUnchanged ? "yes" : "no"}).`;
    } catch (error) {
      exportError = errorMessageFor(error, "RageLab Core rejected the PNG export.");
    } finally {
      exportingTexture = null;
    }
  }

  function textureAuthoringReport(value: unknown): TextureAuthoringReport | null {
    const record = asRecord(value);
    const report = asRecord(record?.authoring);
    if (report?.schema !== "ragelab.texture-authoring") {
      return null;
    }
    return report as unknown as TextureAuthoringReport;
  }

  function materialAuthoringReport(value: unknown): MaterialAuthoringReport | null {
    const record = asRecord(value);
    const report = asRecord(record?.materialAuthoring);
    if (report?.schema !== "ragelab.material-authoring") {
      return null;
    }
    return report as unknown as MaterialAuthoringReport;
  }

  function asRecord(value: unknown): Record<string, unknown> | null {
    return value !== null && typeof value === "object"
      ? (value as Record<string, unknown>)
      : null;
  }

  function defaultPngPath(source: string, texture: TextureAuthoringEntry): string {
    const slash = Math.max(source.lastIndexOf("/"), source.lastIndexOf("\\"));
    const dot = source.lastIndexOf(".");
    const stem = dot > slash ? source.slice(0, dot) : source;
    const safeName = (texture.name || `texture-${texture.index}`).replace(
      /[<>:"/\\|?*]+/g,
      "_",
    );
    return `${stem}-${safeName}.png`;
  }

  function modeLabel(mode: MaterialFieldMode): string {
    return mode === "rebindExisting" ? "Rebind existing" : "Inspect only";
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

  function errorMessageFor(error: unknown, fallback: string): string {
    return error instanceof Error && error.message ? error.message : fallback;
  }
</script>

{#if textureAuthoring}
  <section class="authoring-panel texture-panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">Texture authoring contract</p>
        <h3>{textureAuthoring.textureCount} texture(s)</h3>
      </div>
      <code>{textureAuthoring.schema} v{textureAuthoring.schemaVersion}</code>
    </div>

    <div class="rules">
      {#each textureAuthoring.rules as rule}
        <span>{rule}</span>
      {/each}
    </div>

    {#if exportError}
      <p class="notice error" role="alert">{exportError}</p>
    {/if}
    {#if exportMessage}
      <p class="notice success">{exportMessage}</p>
    {/if}

    <div class="texture-list">
      {#each textureAuthoring.textures as texture}
        <article class="texture-card">
          <div class="texture-title">
            <div>
              <strong>#{texture.index} {texture.name || "(unnamed)"}</strong>
              <span>{texture.nameHash} · dictionary {texture.dictionaryHash}</span>
            </div>
            <button
              type="button"
              onclick={() => exportPng(texture)}
              disabled={!texture.export.pngTopMip || exportingTexture !== null}
            >
              {exportingTexture === texture.index ? "Exporting…" : "Export PNG"}
            </button>
          </div>

          <div class="facts">
            <span>{texture.width}×{texture.height}×{texture.depth}</span>
            <span>{texture.format} ({texture.formatRaw})</span>
            <span>{texture.mipLevels} mip(s)</span>
            <span>{formatBytes(texture.encodedBytes)}</span>
            <span>stride {texture.stride}</span>
          </div>

          <div class="capabilities">
            <span class:enabled={texture.export.pngTopMip}>PNG top mip</span>
            <span class:enabled={texture.export.classicDdsFullMips}>DDS full mips</span>
            <span class:enabled={texture.replacement.png}>PNG replace</span>
            <span class:enabled={texture.replacement.relocatedDds}>DDS relocate</span>
            <span class:enabled={texture.replacement.mipChainRegenerated}>regen mips</span>
            <span class:enabled={texture.replacement.targetFormatPreserved}>format preserved</span>
          </div>

          {#if texture.replacement.reason}
            <p class="reason">{texture.replacement.reason}</p>
          {/if}
        </article>
      {/each}
    </div>
  </section>
{/if}

{#if materialAuthoring}
  <section class="authoring-panel material-panel">
    <div class="panel-heading">
      <div>
        <p class="eyebrow">Material authoring contract</p>
        <h3>{materialAuthoring.assetType} proven fields</h3>
      </div>
      <code>{materialAuthoring.schema} v{materialAuthoring.schemaVersion}</code>
    </div>

    <div class="policy-list">
      {#each materialAuthoring.fieldPolicy as policy}
        <article>
          <div>
            <code>{policy.field}</code>
            <span class:editable={policy.mode === "rebindExisting"} class="mode">
              {modeLabel(policy.mode)}
            </span>
          </div>
          <p>{policy.reason}</p>
        </article>
      {/each}
    </div>

    <div class="drawables">
      {#each materialAuthoring.drawables as drawable}
        <article class="drawable">
          <div class="drawable-heading">
            <strong>
              {drawable.drawableIndex === null ? "" : `#${drawable.drawableIndex} · `}
              {drawable.drawableName}
            </strong>
            <span>
              {drawable.textureBindings.length} texture binding(s) ·
              {drawable.shaderBindings.length} shader binding(s)
            </span>
          </div>

          {#if drawable.editSessionError}
            <p class="notice error">{drawable.editSessionError}</p>
          {/if}

          <div class="binding-grid">
            <div>
              <h4>Texture bindings</h4>
              {#each drawable.textureBindings as binding}
                <div class="binding-row">
                  <code>shader {binding.shaderIndex} / param {binding.parameterIndex}</code>
                  <strong>{binding.textureName}</strong>
                  <span>{binding.parameterHash}</span>
                  <span class="mode editable">{modeLabel(binding.mode)}</span>
                </div>
              {/each}
            </div>

            <div>
              <h4>Shaders</h4>
              {#each drawable.shaders as shader}
                <div class="binding-row">
                  <code>shader {shader.index}</code>
                  <strong>{shader.nameHash}</strong>
                  <span>file {shader.fileHash}</span>
                  <span class="mode">{shader.textureReferences.length} texture ref(s)</span>
                </div>
              {/each}
            </div>
          </div>
        </article>
      {/each}
    </div>
  </section>
{/if}

<style>
  .authoring-panel {
    margin-top: 18px;
    padding: 18px;
    border: 1px solid #303740;
    border-radius: 10px;
    background: rgba(10, 13, 17, 0.72);
  }

  .panel-heading,
  .texture-title,
  .drawable-heading,
  .policy-list article > div {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 16px;
  }

  .eyebrow {
    margin: 0 0 4px;
    color: #8b94a1;
    font-size: 11px;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  h3,
  h4,
  p {
    margin-top: 0;
  }

  .panel-heading h3 {
    margin-bottom: 0;
  }

  .rules,
  .facts,
  .capabilities {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .rules {
    margin: 14px 0;
  }

  .rules span,
  .facts span,
  .capabilities span,
  .mode {
    padding: 4px 7px;
    border: 1px solid #343b45;
    border-radius: 999px;
    color: #aeb6c2;
    font-size: 11px;
  }

  .capabilities span.enabled,
  .mode.editable {
    border-color: #3f7054;
    color: #9bd4ae;
  }

  .texture-list,
  .policy-list,
  .drawables {
    display: grid;
    gap: 10px;
    margin-top: 14px;
  }

  .texture-card,
  .policy-list article,
  .drawable {
    padding: 12px;
    border: 1px solid #292f37;
    border-radius: 8px;
    background: rgba(25, 29, 35, 0.55);
  }

  .texture-title span,
  .drawable-heading span,
  .binding-row span,
  .reason,
  .policy-list p {
    color: #8f98a5;
    font-size: 12px;
  }

  .texture-title > div {
    display: grid;
    gap: 3px;
  }

  button {
    padding: 7px 11px;
    border: 1px solid #47505d;
    border-radius: 7px;
    background: #20262e;
    color: #e7ebf0;
    cursor: pointer;
  }

  button:disabled {
    cursor: default;
    opacity: 0.5;
  }

  .facts,
  .capabilities {
    margin-top: 10px;
  }

  .notice {
    margin: 12px 0 0;
    padding: 9px 11px;
    border-radius: 7px;
    font-size: 12px;
  }

  .notice.error {
    border: 1px solid #77434a;
    background: rgba(90, 35, 43, 0.25);
    color: #efabb4;
  }

  .notice.success {
    border: 1px solid #3d674d;
    background: rgba(34, 83, 53, 0.25);
    color: #a2d7b2;
  }

  .policy-list article > div {
    align-items: center;
  }

  .policy-list p {
    margin: 7px 0 0;
  }

  .binding-grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 12px;
    margin-top: 12px;
  }

  .binding-grid h4 {
    margin-bottom: 7px;
    color: #cbd2db;
    font-size: 12px;
  }

  .binding-row {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.4fr);
    gap: 4px 10px;
    padding: 8px 0;
    border-top: 1px solid #2b3139;
    font-size: 11px;
  }

  @media (max-width: 900px) {
    .binding-grid {
      grid-template-columns: 1fr;
    }

    .panel-heading,
    .texture-title,
    .drawable-heading {
      flex-direction: column;
    }
  }
</style>
