<script lang="ts">
  import {
    applyOperation,
    chooseOperationOutput,
    chooseReplacementFile,
    planOperation,
    type AssetOperationCapability,
    type OperationApplyResult,
    type OperationDocument,
    type OperationPlan,
  } from "$lib/native";
  import {
    buildOperationSpec,
    defaultOperationValues,
    editableOperationCapabilities,
    operationFormDefinition,
    type OperationFormDefinition,
  } from "$lib/operations";

  export let sourcePath = "";
  export let operations: AssetOperationCapability[] = [];

  let editableCapabilities: AssetOperationCapability[] = [];
  let selectedOperationId = "";
  let definition: OperationFormDefinition | null = null;
  let values: Record<string, string> = {};
  let outputPath = "";
  let activeSourcePath = sourcePath;

  let plan: OperationPlan | null = null;
  let plannedDocumentJson = "";
  let applyResult: OperationApplyResult | null = null;
  let planning = false;
  let applying = false;
  let errorMessage = "";

  $: editableCapabilities = editableOperationCapabilities(operations);
  $: if (sourcePath !== activeSourcePath) {
    activeSourcePath = sourcePath;
    outputPath = "";
    selectedOperationId = "";
    definition = null;
    values = {};
    errorMessage = "";
    invalidatePlan();
  }
  $: if (
    editableCapabilities.length > 0 &&
    !editableCapabilities.some(
      (capability) => capability.id === selectedOperationId,
    )
  ) {
    selectOperation(editableCapabilities[0].id);
  }
  $: if (editableCapabilities.length === 0 && selectedOperationId) {
    selectedOperationId = "";
    definition = null;
    values = {};
    invalidatePlan();
  }

  function selectOperation(id: string) {
    selectedOperationId = id;
    definition = operationFormDefinition(id);
    values = definition ? defaultOperationValues(definition) : {};
    invalidatePlan();
  }

  function setField(key: string, value: string) {
    values = {
      ...values,
      [key]: value,
    };
    invalidatePlan();
  }

  async function selectPayload(key: string, label: string) {
    errorMessage = "";
    try {
      const selected = await chooseReplacementFile("Select " + label);
      if (selected) {
        setField(key, selected);
      }
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "Unable to choose the replacement payload.",
      );
    }
  }

  async function selectOutput() {
    errorMessage = "";
    try {
      const selected = await chooseOperationOutput(defaultOutputPath(sourcePath));
      if (selected) {
        outputPath = selected;
        invalidatePlan();
      }
    } catch (error) {
      errorMessage = errorMessageFor(error, "Unable to choose an output path.");
    }
  }

  async function runPlan() {
    errorMessage = "";
    applyResult = null;

    let document: OperationDocument;
    try {
      document = buildDocument();
    } catch (error) {
      errorMessage = errorMessageFor(error, "Operation document is invalid.");
      return;
    }

    planning = true;
    plan = null;
    plannedDocumentJson = "";

    try {
      const result = await planOperation(document);
      plan = result;
      plannedDocumentJson = JSON.stringify(document);
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "RageLab core could not plan this operation.",
      );
    } finally {
      planning = false;
    }
  }

  async function runApply() {
    errorMessage = "";

    let document: OperationDocument;
    try {
      document = buildDocument();
    } catch (error) {
      errorMessage = errorMessageFor(error, "Operation document is invalid.");
      return;
    }

    const currentDocumentJson = JSON.stringify(document);
    if (
      !plan ||
      !plan.allowed ||
      !plannedDocumentJson ||
      currentDocumentJson !== plannedDocumentJson
    ) {
      errorMessage =
        "Run a successful dry-run plan for the current document before applying.";
      return;
    }

    applying = true;
    applyResult = null;

    try {
      applyResult = await applyOperation(document);
    } catch (error) {
      errorMessage = errorMessageFor(
        error,
        "RageLab core rejected or failed the apply request.",
      );
    } finally {
      applying = false;
    }
  }

  function buildDocument(): OperationDocument {
    if (!definition) {
      throw new Error("Choose a writable operation.");
    }
    if (!sourcePath) {
      throw new Error("No source asset is selected.");
    }
    if (!outputPath) {
      throw new Error("Choose a separate output path.");
    }
    if (sourcePath === outputPath) {
      throw new Error("Output must differ from the source path.");
    }

    return {
      schema: "ragelab.operation",
      schemaVersion: 1,
      source: sourcePath,
      output: outputPath,
      operations: [buildOperationSpec(definition, values)],
    };
  }

  function invalidatePlan() {
    plan = null;
    plannedDocumentJson = "";
    applyResult = null;
  }

  function currentCapability(): AssetOperationCapability | null {
    return (
      editableCapabilities.find(
        (capability) => capability.id === selectedOperationId,
      ) ?? null
    );
  }

  function errorMessageFor(error: unknown, fallback: string): string {
    return error instanceof Error && error.message ? error.message : fallback;
  }

  function defaultOutputPath(source: string): string | undefined {
    if (!source) {
      return undefined;
    }

    const slash = Math.max(source.lastIndexOf("/"), source.lastIndexOf("\\"));
    const dot = source.lastIndexOf(".");
    if (dot > slash) {
      return source.slice(0, dot) + "-edited" + source.slice(dot);
    }
    return source + "-edited";
  }
</script>

<section class="editor">
  <div class="heading">
    <div>
      <p class="label">Declarative edit</p>
      <h3>Plan before apply</h3>
      <p class="detail">
        Studio builds a versioned operation document and delegates planning,
        writer eligibility, mutation, semantic reopen, and source verification to
        RageLab core.
      </p>
    </div>

    <span class="safety">Source is never overwritten</span>
  </div>

  {#if editableCapabilities.length === 0}
    <div class="empty">
      No writable declarative operation is available for this asset.
    </div>
  {:else}
    <div class="form-grid">
      <label class="field wide">
        <span>Operation</span>
        <select
          value={selectedOperationId}
          onchange={(event) =>
            selectOperation((event.currentTarget as HTMLSelectElement).value)}
        >
          {#each editableCapabilities as capability}
            <option value={capability.id}>
              {operationFormDefinition(capability.id)?.label ?? capability.id}
            </option>
          {/each}
        </select>
      </label>

      {#if definition}
        {#each definition.fields as field}
          <label class:wide={field.kind === "file"} class="field">
            <span>{field.label}</span>

            {#if field.kind === "select"}
              <select
                value={values[field.key] ?? ""}
                onchange={(event) =>
                  setField(
                    field.key,
                    (event.currentTarget as HTMLSelectElement).value,
                  )}
              >
                {#each field.options ?? [] as option}
                  <option value={option}>{option}</option>
                {/each}
              </select>
            {:else if field.kind === "file"}
              <div class="path-control">
                <code>{values[field.key] || "No payload selected"}</code>
                <button
                  type="button"
                  onclick={() => selectPayload(field.key, field.label)}
                >
                  Choose
                </button>
              </div>
            {:else}
              <input
                type="number"
                value={values[field.key] ?? ""}
                min={field.min}
                step={field.step}
                placeholder={field.placeholder}
                required={field.required}
                oninput={(event) =>
                  setField(
                    field.key,
                    (event.currentTarget as HTMLInputElement).value,
                  )}
              />
            {/if}
          </label>
        {/each}

        <div class="field wide">
          <span>Output</span>
          <div class="path-control">
            <code>{outputPath || "Choose a new output file"}</code>
            <button type="button" onclick={selectOutput}>Choose output</button>
          </div>
        </div>
      {/if}
    </div>

    {#if currentCapability()?.reason}
      <p class="capability-reason">{currentCapability()?.reason}</p>
    {/if}

    {#if definition}
      <p class="operation-description">{definition.description}</p>
    {/if}

    <div class="actions">
      <button
        class="plan"
        type="button"
        onclick={runPlan}
        disabled={planning || applying}
      >
        {planning ? "Planning…" : "Run dry-run plan"}
      </button>

      <button
        class="apply"
        type="button"
        onclick={runApply}
        disabled={!plan?.allowed || planning || applying || applyResult !== null}
      >
        {applying ? "Applying…" : "Apply to new output"}
      </button>
    </div>

    {#if errorMessage}
      <div class="error" role="alert">{errorMessage}</div>
    {/if}

    {#if plan}
      <section class:allowed={plan.allowed} class="plan-result">
        <div class="result-heading">
          <div>
            <span class="result-label">Dry-run plan</span>
            <strong>{plan.allowed ? "Allowed" : "Blocked"}</strong>
          </div>
          <span>{plan.assetType}</span>
        </div>

        <div class="result-grid">
          <div>
            <span>Non-destructive</span>
            <strong>{plan.nonDestructive ? "Yes" : "No"}</strong>
          </div>
          <div>
            <span>Output exists</span>
            <strong>{plan.outputExists ? "Yes" : "No"}</strong>
          </div>
          <div>
            <span>Source bytes</span>
            <strong>{plan.sourceBytes}</strong>
          </div>
          <div>
            <span>Writes</span>
            <strong>{plan.writes.length}</strong>
          </div>
        </div>

        {#if plan.reasons.length > 0}
          <div class="reasons">
            {#each plan.reasons as reason}
              <p>{reason}</p>
            {/each}
          </div>
        {/if}

        <div class="planned-operations">
          {#each plan.operations as operation}
            <article>
              <code>{operation.type}</code>
              <span class:accepted={operation.allowed}>
                {operation.allowed ? "allowed" : "blocked"}
              </span>
              {#if operation.reason}
                <p>{operation.reason}</p>
              {/if}
              {#if operation.details}
                <pre>{JSON.stringify(operation.details, null, 2)}</pre>
              {/if}
            </article>
          {/each}
        </div>
      </section>
    {/if}

    {#if applyResult}
      <section class="apply-result">
        <div class="result-heading">
          <div>
            <span class="result-label">Apply complete</span>
            <strong>{applyResult.assetType}</strong>
          </div>
          <span>{applyResult.operationsApplied} operation(s)</span>
        </div>

        <div class="result-grid">
          <div>
            <span>Bytes written</span>
            <strong>{applyResult.bytesWritten}</strong>
          </div>
          <div>
            <span>Semantic reopen</span>
            <strong>
              {applyResult.validation.semanticReopen ? "Passed" : "Failed"}
            </strong>
          </div>
          <div>
            <span>Source unchanged</span>
            <strong>
              {applyResult.validation.sourceUnchanged ? "Passed" : "Failed"}
            </strong>
          </div>
          <div>
            <span>Non-destructive</span>
            <strong>{applyResult.nonDestructive ? "Yes" : "No"}</strong>
          </div>
        </div>

        <div class="output-path">
          <span>Output</span>
          <code>{applyResult.output}</code>
        </div>
      </section>
    {/if}
  {/if}
</section>

<style>
  .editor {
    margin-top: 28px;
    padding-top: 24px;
    border-top: 1px solid #252a31;
  }

  .heading {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: 24px;
  }

  .label,
  .result-label {
    margin: 0 0 7px;
    color: #7d838c;
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  h3 {
    margin: 0;
    font-size: 17px;
    letter-spacing: -0.02em;
  }

  .detail {
    max-width: 720px;
    margin: 7px 0 0;
    color: #858b95;
    font-size: 12px;
    line-height: 1.55;
  }

  .safety {
    flex: none;
    padding: 5px 8px;
    border: 1px solid #314034;
    border-radius: 6px;
    color: #8cb198;
    background: #121a14;
    font-size: 10px;
    font-weight: 650;
    text-transform: uppercase;
  }

  .empty {
    margin-top: 14px;
    padding: 14px;
    border: 1px solid #292e35;
    border-radius: 8px;
    color: #7f8690;
    background: #12151a;
    font-size: 12px;
  }

  .form-grid {
    margin-top: 18px;
    display: grid;
    grid-template-columns: repeat(3, minmax(0, 1fr));
    gap: 10px;
  }

  .field {
    min-width: 0;
    display: grid;
    align-content: start;
    gap: 6px;
  }

  .field.wide {
    grid-column: 1 / -1;
  }

  .field > span {
    color: #7f8690;
    font-size: 10px;
    font-weight: 600;
  }

  input,
  select {
    min-width: 0;
    border: 1px solid #343941;
    border-radius: 6px;
    padding: 9px 10px;
    background: #0f1216;
    color: #e5e7eb;
    font: inherit;
    font-size: 12px;
  }

  .path-control {
    min-height: 36px;
    border: 1px solid #343941;
    border-radius: 6px;
    background: #0f1216;
    display: flex;
    align-items: center;
    gap: 10px;
    overflow: hidden;
  }

  .path-control code {
    min-width: 0;
    flex: 1;
    padding: 8px 10px;
    overflow-wrap: anywhere;
    color: #b8bec7;
    font-size: 10px;
  }

  .path-control button {
    align-self: stretch;
    flex: none;
    border: 0;
    border-left: 1px solid #343941;
    padding: 0 11px;
    background: #191d22;
    color: #c7cbd1;
    cursor: pointer;
  }

  .path-control button:hover {
    background: #20252b;
  }

  .capability-reason,
  .operation-description {
    margin: 10px 0 0;
    color: #858b95;
    font-size: 11px;
    line-height: 1.5;
  }

  .actions {
    margin-top: 16px;
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .actions button {
    border-radius: 7px;
    padding: 9px 12px;
    font: inherit;
    font-size: 12px;
    font-weight: 650;
    cursor: pointer;
  }

  .plan {
    border: 1px solid #3a4049;
    background: #1a1e24;
    color: #dce0e5;
  }

  .apply {
    border: 1px solid #35503d;
    background: #172119;
    color: #9fc5aa;
  }

  .actions button:disabled {
    cursor: default;
    opacity: 0.42;
  }

  .error {
    margin-top: 12px;
    padding: 11px 13px;
    border: 1px solid #493027;
    border-radius: 8px;
    background: #1c1411;
    color: #d2a187;
    font-size: 12px;
  }

  .plan-result,
  .apply-result {
    margin-top: 14px;
    padding: 14px;
    border: 1px solid #493027;
    border-radius: 9px;
    background: #171310;
  }

  .plan-result.allowed,
  .apply-result {
    border-color: #304337;
    background: #121914;
  }

  .result-heading {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 18px;
  }

  .result-heading strong {
    display: block;
    font-size: 17px;
  }

  .result-heading > span {
    color: #7f8690;
    font-size: 11px;
  }

  .result-grid {
    margin-top: 12px;
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 8px;
  }

  .result-grid > div {
    padding: 9px;
    border: 1px solid #29312b;
    border-radius: 7px;
    background: rgba(9, 12, 10, 0.4);
    display: grid;
    gap: 5px;
  }

  .result-grid span {
    color: #737b75;
    font-size: 9px;
    text-transform: uppercase;
  }

  .result-grid strong {
    font-size: 12px;
  }

  .reasons {
    margin-top: 10px;
    padding: 9px 10px;
    border: 1px solid #493027;
    border-radius: 7px;
    color: #c49a83;
    background: #1b1310;
  }

  .reasons p {
    margin: 0;
    font-size: 11px;
    line-height: 1.5;
  }

  .reasons p + p {
    margin-top: 5px;
  }

  .planned-operations {
    margin-top: 10px;
    display: grid;
    gap: 7px;
  }

  .planned-operations article {
    padding: 10px;
    border: 1px solid #2c322e;
    border-radius: 7px;
    background: #0f1411;
  }

  .planned-operations article > span {
    margin-left: 8px;
    color: #b77e72;
    font-size: 10px;
  }

  .planned-operations article > span.accepted {
    color: #8db79b;
  }

  .planned-operations p {
    margin: 7px 0 0;
    color: #a98276;
    font-size: 11px;
  }

  .planned-operations pre {
    margin: 8px 0 0;
    overflow: auto;
    color: #8c948d;
    white-space: pre-wrap;
    font-size: 10px;
  }

  .output-path {
    margin-top: 10px;
    display: grid;
    gap: 5px;
  }

  .output-path span {
    color: #737b75;
    font-size: 9px;
    text-transform: uppercase;
  }

  .output-path code {
    overflow-wrap: anywhere;
    color: #a9b6ac;
    font-size: 10px;
  }

  @media (max-width: 800px) {
    .heading {
      display: grid;
    }

    .safety {
      justify-self: start;
    }

    .form-grid {
      grid-template-columns: 1fr;
    }

    .field.wide {
      grid-column: auto;
    }

    .result-grid {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
