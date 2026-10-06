<script lang="ts">
  import { onDestroy } from "svelte";

  import {
    applyYmapAuthoring,
    chooseYmapAuthoringOutput,
    closeYmapAuthoring,
    openYmapAuthoring,
    previewYmapAuthoring,
    previewYmapAuthoringWorldOverlay,
    redoYmapAuthoring,
    revertYmapAuthoring,
    saveYmapAuthoringAs,
    setNativeViewportGizmo,
    undoYmapAuthoring,
    type NativeViewportSceneRequest,
    type SceneManifestReport,
    type YmapAuthoringEditCommand,
    type YmapAuthoringEntity,
    type YmapAuthoringSnapshot,
  } from "$lib/native";

  export let scene: NativeViewportSceneRequest;
  export let manifest: SceneManifestReport;
  export let worldBrowserActive = false;
  export let selectedNodeIndex: number | null = null;
  export let onSnapshot: ((snapshot: YmapAuthoringSnapshot) => void) | null = null;

  type GizmoMode = "translate" | "rotate" | "scale";

  let snapshot: YmapAuthoringSnapshot | null = null;
  let busy = false;
  let errorMessage = "";
  let sessionOpen = false;
  let mode: GizmoMode = "translate";
  let multiMode = false;
  let selectedIndices: number[] = [];
  let selectionText = "";
  let lastPrimaryIndex: number | null = null;
  let lastFormKey = "";

  let px = "0";
  let py = "0";
  let pz = "0";
  let rx = "0";
  let ry = "0";
  let rz = "0";
  let scaleXy = "1";
  let scaleZ = "1";
  let translateSnap = "0.25";
  let rotateSnap = "5";
  let scaleSnap = "0.05";
  let createArchetypeHash = "";

  $: selectedNode =
    manifest.nodes.find((node) => node.index === selectedNodeIndex) ?? null;
  $: primaryEntityIndex = selectedNode?.entityIndex ?? null;
  $: primaryEntity =
    primaryEntityIndex === null
      ? null
      : snapshot?.entities.find((entity) => entity.index === primaryEntityIndex) ?? null;

  $: if (primaryEntityIndex !== lastPrimaryIndex) {
    if (primaryEntityIndex !== null) {
      selectedIndices = multiMode
        ? uniqueIndices([...selectedIndices, primaryEntityIndex])
        : [primaryEntityIndex];
      selectionText = selectedIndices.join(", ");
    } else if (!multiMode) {
      selectedIndices = [];
      selectionText = "";
    }
    lastPrimaryIndex = primaryEntityIndex;
  }

  $: {
    const key =
      primaryEntity && snapshot
        ? snapshot.report.revision + ":" + primaryEntity.index
        : "";
    if (primaryEntity && key !== lastFormKey) {
      syncForm(primaryEntity);
      lastFormKey = key;
    }
  }

  onDestroy(() => {
    void setNativeViewportGizmo(null).catch(() => {});
    if (sessionOpen) {
      void closeYmapAuthoring().catch(() => {});
    }
  });

  async function startEditing() {
    if (busy) return;
    busy = true;
    errorMessage = "";
    try {
      await updateSnapshot(await openYmapAuthoring(scene), false);
      sessionOpen = true;
      await previewCurrent();
      await setNativeViewportGizmo(mode);
    } catch (error) {
      setError(error);
    } finally {
      busy = false;
    }
  }

  async function setMode(next: GizmoMode) {
    mode = next;
    if (!sessionOpen) return;
    try {
      await setNativeViewportGizmo(next);
    } catch (error) {
      setError(error);
    }
  }

  async function applyCommands(commands: YmapAuthoringEditCommand[]) {
    if (!sessionOpen || busy || commands.length === 0) return;
    busy = true;
    errorMessage = "";
    try {
      await updateSnapshot(await applyYmapAuthoring(commands), true);
    } catch (error) {
      setError(error);
    } finally {
      busy = false;
    }
  }

  async function undo() {
    await mutateHistory(undoYmapAuthoring);
  }

  async function redo() {
    await mutateHistory(redoYmapAuthoring);
  }

  async function revert() {
    if (!snapshot?.report.dirty) return;
    if (!window.confirm("Revert all unsaved YMAP edits in this session?")) return;
    await mutateHistory(revertYmapAuthoring);
  }

  async function mutateHistory(action: () => Promise<YmapAuthoringSnapshot>) {
    if (!sessionOpen || busy) return;
    busy = true;
    errorMessage = "";
    try {
      await updateSnapshot(await action(), true);
    } catch (error) {
      setError(error);
    } finally {
      busy = false;
    }
  }

  async function saveAs() {
    if (!sessionOpen || busy || !snapshot) return;
    errorMessage = "";
    try {
      const output = await chooseYmapAuthoringOutput();
      if (!output) return;
      busy = true;
      const result = await saveYmapAuthoringAs(output);
      await updateSnapshot(result.snapshot, false);
    } catch (error) {
      setError(error);
    } finally {
      busy = false;
    }
  }

  async function updateSnapshot(next: YmapAuthoringSnapshot, preview: boolean) {
    snapshot = next;
    manifest = next.manifest;
    onSnapshot?.(next);
    selectionText = selectedIndices.join(", ");
    if (preview) {
      await previewCurrent();
    }
  }

  async function previewCurrent() {
    if (worldBrowserActive) {
      await previewYmapAuthoringWorldOverlay();
    } else {
      await previewYmapAuthoring();
    }
  }

  function setSelectionFromText() {
    const values = selectionText
      .split(/[\s,;]+/)
      .map((value) => Number.parseInt(value, 10))
      .filter((value) => Number.isInteger(value) && value >= 0);
    const max = snapshot?.entities.length ?? 0;
    selectedIndices = uniqueIndices(values).filter((value) => value < max);
    selectionText = selectedIndices.join(", ");
  }

  function selectCurrentOnly() {
    selectedIndices =
      primaryEntityIndex === null ? [] : [primaryEntityIndex];
    selectionText = selectedIndices.join(", ");
  }

  function selectedEntities(): YmapAuthoringEntity[] {
    if (!snapshot) return [];
    const wanted = new Set(selectedIndices);
    return snapshot.entities.filter((entity) => wanted.has(entity.index));
  }

  async function applyNumericTransform() {
    if (!primaryEntity || !snapshot) return;
    const nextPosition: [number, number, number] = [
      finiteNumber(px, "X"),
      finiteNumber(py, "Y"),
      finiteNumber(pz, "Z"),
    ];
    const nextEuler: [number, number, number] = [
      finiteNumber(rx, "RX"),
      finiteNumber(ry, "RY"),
      finiteNumber(rz, "RZ"),
    ];
    const nextRotation = eulerDegreesToQuat(nextEuler);
    const nextScaleXy = finiteNumber(scaleXy, "Scale XY");
    const nextScaleZ = finiteNumber(scaleZ, "Scale Z");

    const translationDelta: [number, number, number] = [
      nextPosition[0] - primaryEntity.position[0],
      nextPosition[1] - primaryEntity.position[1],
      nextPosition[2] - primaryEntity.position[2],
    ];
    const rotationDelta = quatMultiply(
      nextRotation,
      quatConjugate(primaryEntity.rotation),
    );
    const xyFactor =
      Math.abs(primaryEntity.scaleXy ?? 1) > 1e-6
        ? nextScaleXy / (primaryEntity.scaleXy ?? 1)
        : 1;
    const zFactor =
      Math.abs(primaryEntity.scaleZ ?? 1) > 1e-6
        ? nextScaleZ / (primaryEntity.scaleZ ?? 1)
        : 1;

    const entities = selectedEntities();
    const commands = entities.map<YmapAuthoringEditCommand>((entity) => ({
      type: "setTransform",
      index: entity.index,
      position:
        entity.index === primaryEntity.index
          ? nextPosition
          : add3(entity.position, translationDelta),
      rotation:
        entity.index === primaryEntity.index
          ? nextRotation
          : normalizeQuat(quatMultiply(rotationDelta, entity.rotation)),
      scaleXy:
        entity.index === primaryEntity.index
          ? nextScaleXy
          : (entity.scaleXy ?? 1) * xyFactor,
      scaleZ:
        entity.index === primaryEntity.index
          ? nextScaleZ
          : (entity.scaleZ ?? 1) * zFactor,
    }));
    await applyCommands(commands);
  }

  async function nudge(axis: "x" | "y" | "z" | "xy", direction: -1 | 1) {
    const entities = selectedEntities();
    if (entities.length === 0) return;

    if (mode === "translate") {
      const step = positiveNumber(translateSnap, "Translate snap") * direction;
      const delta: [number, number, number] = [
        axis === "x" ? step : 0,
        axis === "y" ? step : 0,
        axis === "z" ? step : 0,
      ];
      await applyCommands(
        entities.map((entity) => ({
          type: "setTransform",
          index: entity.index,
          position: add3(entity.position, delta),
          rotation: entity.rotation,
          scaleXy: entity.scaleXy,
          scaleZ: entity.scaleZ,
        })),
      );
      return;
    }

    if (mode === "rotate") {
      if (axis === "xy") return;
      const degrees = positiveNumber(rotateSnap, "Rotate snap") * direction;
      const delta = axisAngleQuat(axis, degrees);
      await applyCommands(
        entities.map((entity) => ({
          type: "setTransform",
          index: entity.index,
          position: entity.position,
          rotation: normalizeQuat(quatMultiply(delta, entity.rotation)),
          scaleXy: entity.scaleXy,
          scaleZ: entity.scaleZ,
        })),
      );
      return;
    }

    const step = positiveNumber(scaleSnap, "Scale snap") * direction;
    await applyCommands(
      entities.map((entity) => {
        const xy = Math.max(0.001, (entity.scaleXy ?? 1) + (axis === "xy" ? step : 0));
        const z = Math.max(0.001, (entity.scaleZ ?? 1) + (axis === "z" ? step : 0));
        return {
          type: "setTransform",
          index: entity.index,
          position: entity.position,
          rotation: entity.rotation,
          scaleXy: xy,
          scaleZ: z,
        };
      }),
    );
  }

  async function duplicateSelection() {
    const indices = selectedIndices;
    if (indices.length === 0) return;
    const step = positiveNumber(translateSnap, "Translate snap");
    await applyCommands([
      {
        type: "duplicate",
        indices,
        translation: [step, 0, 0],
      },
    ]);
    if (snapshot) {
      const count = indices.length;
      const start = Math.max(0, snapshot.entities.length - count);
      selectedIndices = Array.from({ length: count }, (_, offset) => start + offset);
      selectionText = selectedIndices.join(", ");
    }
  }

  async function deleteSelection() {
    if (selectedIndices.length === 0) return;
    if (!window.confirm("Delete selected YMAP entities? Parent references are guarded by Core.")) {
      return;
    }
    await applyCommands([{ type: "delete", indices: selectedIndices }]);
    selectedIndices = [];
    selectionText = "";
  }

  async function createFromTemplate() {
    if (!primaryEntity) return;
    await placeArchetype(
      parseHash(createArchetypeHash || String(primaryEntity.archetypeHash)),
    );
  }

  export async function placeArchetype(
    archetypeHash: number,
    position?: [number, number, number],
  ) {
    if (!sessionOpen) {
      await startEditing();
    }
    if (!sessionOpen || !snapshot) {
      throw new Error("Open an authoring session before placing an archetype.");
    }
    const template = primaryEntity ?? snapshot.entities[0];
    if (!template) {
      throw new Error("Template-based placement requires at least one YMAP entity.");
    }

    const step = positiveNumber(translateSnap, "Translate snap");
    const target: [number, number, number] =
      position ?? [template.position[0] + step, template.position[1], template.position[2]];
    const beforeCount = snapshot.entities.length;
    await applyCommands([
      {
        type: "create",
        templateIndex: template.index,
        archetypeHash,
        position: target,
        rotation: template.rotation,
        scaleXy: template.scaleXy,
        scaleZ: template.scaleZ,
        flags: template.flags,
        parentIndex: template.parentIndex,
      },
    ]);

    if (!snapshot || snapshot.entities.length <= beforeCount) {
      return;
    }
    const created = snapshot.entities.at(-1);
    if (!created) return;
    selectedIndices = [created.index];
    selectionText = selectedIndices.join(", ");
    const rendered = snapshot.manifest.nodes.find(
      (node) => node.entityIndex === created.index,
    );
    selectedNodeIndex = rendered?.index ?? null;
  }

  function syncForm(entity: YmapAuthoringEntity) {
    [px, py, pz] = entity.position.map(formatNumber) as [string, string, string];
    const euler = quatToEulerDegrees(entity.rotation);
    [rx, ry, rz] = euler.map(formatNumber) as [string, string, string];
    scaleXy = formatNumber(entity.scaleXy ?? 1);
    scaleZ = formatNumber(entity.scaleZ ?? 1);
    createArchetypeHash = "0x" + entity.archetypeHash.toString(16).padStart(8, "0").toUpperCase();
  }

  function finiteNumber(value: string, label: string) {
    const parsed = Number(value);
    if (!Number.isFinite(parsed)) throw new Error(label + " must be finite.");
    return parsed;
  }

  function positiveNumber(value: string, label: string) {
    const parsed = finiteNumber(value, label);
    if (parsed <= 0) throw new Error(label + " must be greater than zero.");
    return parsed;
  }

  function parseHash(value: string) {
    const trimmed = value.trim();
    const parsed = /^0x/i.test(trimmed)
      ? Number.parseInt(trimmed.slice(2), 16)
      : Number.parseInt(trimmed, 10);
    if (!Number.isInteger(parsed) || parsed < 0 || parsed > 0xffffffff) {
      throw new Error("Archetype hash must be a uint32 decimal or 0x hexadecimal value.");
    }
    return parsed >>> 0;
  }

  function uniqueIndices(values: number[]) {
    return [...new Set(values)];
  }

  function add3(
    left: [number, number, number],
    right: [number, number, number],
  ): [number, number, number] {
    return [left[0] + right[0], left[1] + right[1], left[2] + right[2]];
  }

  function axisAngleQuat(axis: "x" | "y" | "z", degrees: number): [number, number, number, number] {
    const half = (degrees * Math.PI) / 360;
    const s = Math.sin(half);
    const c = Math.cos(half);
    return axis === "x" ? [s, 0, 0, c] : axis === "y" ? [0, s, 0, c] : [0, 0, s, c];
  }

  function quatMultiply(
    a: [number, number, number, number],
    b: [number, number, number, number],
  ): [number, number, number, number] {
    const [ax, ay, az, aw] = a;
    const [bx, by, bz, bw] = b;
    return [
      aw * bx + ax * bw + ay * bz - az * by,
      aw * by - ax * bz + ay * bw + az * bx,
      aw * bz + ax * by - ay * bx + az * bw,
      aw * bw - ax * bx - ay * by - az * bz,
    ];
  }

  function quatConjugate(q: [number, number, number, number]): [number, number, number, number] {
    return [-q[0], -q[1], -q[2], q[3]];
  }

  function normalizeQuat(q: [number, number, number, number]): [number, number, number, number] {
    const length = Math.hypot(q[0], q[1], q[2], q[3]);
    if (!Number.isFinite(length) || length <= 1e-9) return [0, 0, 0, 1];
    return [q[0] / length, q[1] / length, q[2] / length, q[3] / length];
  }

  function eulerDegreesToQuat(
    degrees: [number, number, number],
  ): [number, number, number, number] {
    const [x, y, z] = degrees.map((value) => (value * Math.PI) / 180);
    const cx = Math.cos(x / 2);
    const sx = Math.sin(x / 2);
    const cy = Math.cos(y / 2);
    const sy = Math.sin(y / 2);
    const cz = Math.cos(z / 2);
    const sz = Math.sin(z / 2);
    return normalizeQuat([
      sx * cy * cz - cx * sy * sz,
      cx * sy * cz + sx * cy * sz,
      cx * cy * sz - sx * sy * cz,
      cx * cy * cz + sx * sy * sz,
    ]);
  }

  function quatToEulerDegrees(
    q: [number, number, number, number],
  ): [number, number, number] {
    const [x, y, z, w] = normalizeQuat(q);
    const sinr = 2 * (w * x + y * z);
    const cosr = 1 - 2 * (x * x + y * y);
    const roll = Math.atan2(sinr, cosr);
    const sinp = 2 * (w * y - z * x);
    const pitch = Math.abs(sinp) >= 1 ? Math.sign(sinp) * Math.PI / 2 : Math.asin(sinp);
    const siny = 2 * (w * z + x * y);
    const cosy = 1 - 2 * (y * y + z * z);
    const yaw = Math.atan2(siny, cosy);
    const scale = 180 / Math.PI;
    return [roll * scale, pitch * scale, yaw * scale];
  }

  function formatNumber(value: number) {
    return Number(value.toFixed(4)).toString();
  }

  function setError(error: unknown) {
    errorMessage = error instanceof Error ? error.message : String(error ?? "Authoring failed.");
  }
</script>

<section class="authoring-panel" class:dirty={snapshot?.report.dirty}>
  <div class="authoring-heading">
    <div>
      <p class="label">YMAP authoring</p>
      <strong>{sessionOpen ? "Edit session" : "Read-only scene"}</strong>
    </div>
    {#if !sessionOpen}
      <button type="button" onclick={() => void startEditing()} disabled={busy}>
        {busy ? "Opening…" : "Start editing"}
      </button>
    {:else}
      <div class="history-actions">
        <button type="button" onclick={() => void undo()} disabled={busy || !snapshot?.report.undoDepth}>Undo</button>
        <button type="button" onclick={() => void redo()} disabled={busy || !snapshot?.report.redoDepth}>Redo</button>
        <button type="button" onclick={() => void revert()} disabled={busy || !snapshot?.report.dirty}>Revert</button>
        <button type="button" onclick={() => void saveAs()} disabled={busy || !snapshot?.report.dirty || snapshot?.report.sourceConflict}>Save As</button>
      </div>
    {/if}
  </div>

  {#if sessionOpen && snapshot}
    <div class="status-line">
      <span class:active={snapshot.report.dirty}>{snapshot.report.dirty ? "dirty" : "clean"}</span>
      <span>rev {snapshot.report.revision}</span>
      <span>{snapshot.report.entities} entities</span>
      <span>undo {snapshot.report.undoDepth}</span>
      <span>redo {snapshot.report.redoDepth}</span>
      {#if snapshot.report.sourceConflict}
        <span class="conflict">source conflict</span>
      {/if}
    </div>

    <div class="selection-row">
      <label>
        <span>Selection</span>
        <input bind:value={selectionText} onblur={setSelectionFromText} placeholder="0, 1, 2" />
      </label>
      <label class="check"><input type="checkbox" bind:checked={multiMode} /> additive pick</label>
      <button type="button" onclick={selectCurrentOnly} disabled={primaryEntityIndex === null}>Current only</button>
      <span>{selectedIndices.length} selected</span>
    </div>

    {#if primaryEntity}
      <div class="gizmo-tabs">
        <button type="button" class:active={mode === "translate"} onclick={() => void setMode("translate")}>Move</button>
        <button type="button" class:active={mode === "rotate"} onclick={() => void setMode("rotate")}>Rotate</button>
        <button type="button" class:active={mode === "scale"} onclick={() => void setMode("scale")}>Scale</button>
      </div>

      <div class="snap-grid">
        <label><span>Move snap</span><input type="number" min="0.0001" step="0.05" bind:value={translateSnap} /></label>
        <label><span>Rotate snap °</span><input type="number" min="0.01" step="1" bind:value={rotateSnap} /></label>
        <label><span>Scale snap</span><input type="number" min="0.0001" step="0.01" bind:value={scaleSnap} /></label>
      </div>

      <div class="axis-gizmo">
        {#if mode === "scale"}
          <div><span>XY</span><button type="button" onclick={() => void nudge("xy", -1)}>−</button><button type="button" onclick={() => void nudge("xy", 1)}>+</button></div>
          <div><span>Z</span><button type="button" onclick={() => void nudge("z", -1)}>−</button><button type="button" onclick={() => void nudge("z", 1)}>+</button></div>
        {:else}
          {#each ["x", "y", "z"] as axis}
            <div>
              <span>{axis.toUpperCase()}</span>
              <button type="button" onclick={() => void nudge(axis as "x" | "y" | "z", -1)}>−</button>
              <button type="button" onclick={() => void nudge(axis as "x" | "y" | "z", 1)}>+</button>
            </div>
          {/each}
        {/if}
      </div>

      <div class="numeric-grid">
        <label><span>X</span><input type="number" step="any" bind:value={px} /></label>
        <label><span>Y</span><input type="number" step="any" bind:value={py} /></label>
        <label><span>Z</span><input type="number" step="any" bind:value={pz} /></label>
        <label><span>RX °</span><input type="number" step="any" bind:value={rx} /></label>
        <label><span>RY °</span><input type="number" step="any" bind:value={ry} /></label>
        <label><span>RZ °</span><input type="number" step="any" bind:value={rz} /></label>
        <label><span>Scale XY</span><input type="number" step="any" bind:value={scaleXy} /></label>
        <label><span>Scale Z</span><input type="number" step="any" bind:value={scaleZ} /></label>
      </div>
      <button class="apply-transform" type="button" onclick={() => void applyNumericTransform()} disabled={busy}>
        Apply transform to {selectedIndices.length || 1}
      </button>

      <div class="entity-actions">
        <button type="button" onclick={() => void duplicateSelection()} disabled={busy || selectedIndices.length === 0}>Duplicate +X</button>
        <button type="button" onclick={() => void deleteSelection()} disabled={busy || selectedIndices.length === 0}>Delete</button>
      </div>

      <div class="placement">
        <label>
          <span>Archetype hash for placement</span>
          <input bind:value={createArchetypeHash} placeholder="0x12345678" />
        </label>
        <button type="button" onclick={() => void createFromTemplate()} disabled={busy}>
          Place from selected template
        </button>
      </div>
    {:else}
      <p class="hint">Select a rendered entity to edit its local YMAP transform.</p>
    {/if}
  {/if}

  {#if errorMessage}
    <div class="authoring-error" role="alert">{errorMessage}</div>
  {/if}
</section>

<style>
  .authoring-panel {
    margin: 12px 0;
    padding: 10px;
    border: 1px solid #30363f;
    border-radius: 8px;
    background: #101419;
    display: grid;
    gap: 9px;
  }

  .authoring-panel.dirty {
    border-color: #675932;
  }

  .authoring-heading,
  .selection-row,
  .history-actions,
  .gizmo-tabs,
  .axis-gizmo,
  .entity-actions,
  .status-line {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-wrap: wrap;
  }

  .authoring-heading {
    justify-content: space-between;
  }

  .label,
  label > span {
    display: block;
    margin: 0 0 3px;
    color: #737b85;
    font-size: 9px;
    font-weight: 650;
    text-transform: uppercase;
  }

  .authoring-heading strong {
    color: #c2c8d0;
    font-size: 12px;
  }

  button {
    border: 1px solid #353c45;
    border-radius: 5px;
    padding: 5px 7px;
    background: #181d23;
    color: #a9b0ba;
    font: inherit;
    font-size: 10px;
    cursor: pointer;
  }

  button:hover,
  button.active {
    border-color: #617181;
    background: #202830;
    color: #d4dce5;
  }

  button:disabled {
    cursor: default;
    opacity: 0.42;
  }

  .status-line span {
    padding: 2px 5px;
    border: 1px solid #30363f;
    border-radius: 4px;
    color: #828b96;
    font-size: 9px;
  }

  .status-line span.active {
    border-color: #66582f;
    color: #d4bd76;
  }

  .status-line .conflict {
    border-color: #6e3b35;
    color: #dc9185;
  }

  .selection-row label:first-child,
  .placement label {
    flex: 1 1 120px;
  }

  .selection-row > span,
  .check {
    color: #7f8791;
    font-size: 9px;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  input {
    width: 100%;
    min-width: 0;
    box-sizing: border-box;
    border: 1px solid #343b44;
    border-radius: 5px;
    padding: 5px 6px;
    background: #0d1115;
    color: #d6dbe2;
    font: 10px "SFMono-Regular", Consolas, monospace;
  }

  .snap-grid,
  .numeric-grid {
    display: grid;
    gap: 6px;
  }

  .snap-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .numeric-grid {
    grid-template-columns: repeat(3, minmax(0, 1fr));
  }

  .axis-gizmo > div {
    display: grid;
    grid-template-columns: 22px 28px 28px;
    align-items: center;
    gap: 3px;
  }

  .axis-gizmo span {
    color: #89929c;
    font-size: 9px;
    font-weight: 700;
    text-align: center;
  }

  .apply-transform {
    width: 100%;
  }

  .placement {
    padding-top: 7px;
    border-top: 1px solid #252b32;
    display: flex;
    align-items: end;
    gap: 6px;
  }

  .hint {
    margin: 0;
    color: #707984;
    font-size: 10px;
    line-height: 1.4;
  }

  .authoring-error {
    padding: 7px 8px;
    border: 1px solid #58372f;
    border-radius: 5px;
    background: #1b1311;
    color: #d4a093;
    font-size: 10px;
  }
</style>
