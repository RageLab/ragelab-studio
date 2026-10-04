import { invoke } from "@tauri-apps/api/core";
import { join } from "@tauri-apps/api/path";
import { open, save } from "@tauri-apps/plugin-dialog";

export interface StudioInfo {
  product: string;
  version: string;
  coreRevision: string;
}

export type GtaVEdition = "legacy" | "enhanced" | "ambiguous" | "unknown";

export type GtaVDiscoverySource =
  | "explicitPath"
  | "environment"
  | "rockstarRegistry"
  | "steamManifest"
  | "commonPath";

export interface GtaVDiscoveryProvenance {
  source: GtaVDiscoverySource;
  reference: string;
}

export interface GtaVInstallCheck {
  id: string;
  required: boolean;
  passed: boolean;
  path: string;
}

export interface GtaVInstallation {
  root: string;
  edition: GtaVEdition;
  valid: boolean;
  checks: GtaVInstallCheck[];
  provenance: GtaVDiscoveryProvenance[];
  steamAppId?: number;
  steamBuildId?: string;
  steamName?: string;
}

export interface GtaVDiscoveryReport {
  schema: "ragelab.gta.discovery";
  schemaVersion: number;
  platform: string;
  target: {
    product: string;
    edition: string;
    steamAppId: number;
  };
  validLegacyInstallations: number;
  candidates: GtaVInstallation[];
}

export interface GtaRpfKeyCacheReport {
  schema: "ragelab.gta.rpf-keys";
  schemaVersion: number;
  executable: string;
  cache: string;
  cacheHit: boolean;
}

export interface GtaRpfIndexBuildReport {
  schema: "ragelab.gta.rpf-index";
  schemaVersion: number;
  gameRoot: string;
  orderedArchives: number;
  scannedArchives: number;
  nestedArchives: number;
  indexedFiles: number;
  parsedYtyps: number;
  archetypes: number;
  fileKeys: number;
  warnings: string[];
  platformPacks: string[];
}

export interface GtaRpfIndexCacheReport {
  schema: "ragelab.gta.rpf-index-cache";
  schemaVersion: number;
  index: string;
  cacheHit: boolean;
  fingerprint: {
    gtaExeSize: number;
    gtaExeModified: number;
    updateRpfSize: number;
    updateRpfModified: number;
    outerArchiveCount: number;
    outerArchiveSignature: number;
  };
  build: GtaRpfIndexBuildReport | null;
}

export type FiveMDiscoverySource =
  | "explicitPath"
  | "environment"
  | "localAppData";

export type FiveMGtaRelationshipStatus =
  | "missingConfiguration"
  | "invalidConfiguredPath"
  | "validLegacy"
  | "invalidLegacy"
  | "enhanced"
  | "ambiguous"
  | "unknown";

export interface FiveMDiscoveryProvenance {
  source: FiveMDiscoverySource;
  reference: string;
}

export interface FiveMInstallCheck {
  id: string;
  required: boolean;
  passed: boolean;
  path: string;
}

export interface FiveMStoragePath {
  id: string;
  path: string;
  exists: boolean;
}

export interface FiveMGtaRelationship {
  status: FiveMGtaRelationshipStatus;
  configuredPath?: string;
  evidence?: string;
  installation?: GtaVInstallation;
}

export interface FiveMInstallation {
  root: string;
  appRoot: string;
  valid: boolean;
  checks: FiveMInstallCheck[];
  storagePaths: FiveMStoragePath[];
  provenance: FiveMDiscoveryProvenance[];
  citizenFxIni: string;
  savedBuildNumber?: string;
  updateChannel?: string;
  gta: FiveMGtaRelationship;
}

export interface FiveMDiscoveryReport {
  schema: "ragelab.fivem.discovery";
  schemaVersion: number;
  platform: string;
  validInstallations: number;
  legacyLinkedInstallations: number;
  candidates: FiveMInstallation[];
}

export interface AssetContainerInspection {
  kind: "RSC7";
  version: number;
  systemFlags: string;
  graphicsFlags: string;
  systemSize: number;
  graphicsSize: number;
  decompression: boolean;
}

export interface AssetInspectionReport {
  path: string;
  type: string;
  bytes: number;
  container: AssetContainerInspection | null;
  details: unknown;
}

export type AssetOperationAvailability =
  | "available"
  | "parameterized"
  | "contextRequired"
  | "unavailable";

export interface AssetOperationCapability {
  id: string;
  writesAsset: boolean;
  structuredOutput: boolean;
  requiresWorkspace: boolean;
  availability: AssetOperationAvailability;
  reason: string | null;
  requiresParameters: string[];
}

export interface AssetCapabilitiesReport {
  path: string;
  type: string;
  operations: AssetOperationCapability[];
}

export interface PreviewSpatial {
  classification: "localOnly";
  coordinateConvention: string;
}

export interface AssetPreviewReport {
  path: string;
  type: "YDR" | "YDD" | "YFT" | "YBN";
  spatial: PreviewSpatial;
  preview: Record<string, unknown>;
}

export interface CorePreviewOptions {
  drawableIndex?: number;
  maxPrimitives?: number;
  maxVertices?: number;
  maxIndices?: number;
  maxShaders?: number;
  maxTextureReferences?: number;
  maxChildren?: number;
  maxMaterials?: number;
}

export type SceneResolutionState = "resolved" | "unresolved";

export type SceneResolutionReasonCode =
  | "providerMissing"
  | "providerAmbiguous"
  | "assetNameMissing"
  | "drawableDictionaryMissing"
  | "assetMissing"
  | "assetAmbiguous"
  | "dictionaryUnreadable"
  | "dictionaryEntryMissing"
  | "unsupportedAssetRelation"
  | "invalidWorldTransform";

export type SceneCollisionState = "localOnly" | "unresolved";

export interface SceneTransformReport {
  translation: [number, number, number];
  rotation: [number, number, number, number];
  scale: [number, number, number] | null;
}

export interface SceneResolutionReasonReport {
  code: SceneResolutionReasonCode;
  message: string;
}

export interface SceneCollisionRelationshipReport {
  hash: string;
  assetRef: number | null;
  state: SceneCollisionState;
  reason: string;
}

export interface SceneNodeReport {
  index: number;
  sourceYmap: string;
  entityIndex: number;
  archetypeHash: string;
  providerPath: string | null;
  assetRef: number | null;
  assetKind: string | null;
  transform: SceneTransformReport | null;
  resolution: SceneResolutionState;
  reason: SceneResolutionReasonReport | null;
  collision: SceneCollisionRelationshipReport | null;
}

export interface SceneAssetSelectorReport {
  type: "yddDrawable";
  index: number;
  nameHash: string;
  name: string | null;
}

export interface SceneAssetReferenceReport {
  id: number;
  kind: string;
  hash: string;
  path: string;
  selector: SceneAssetSelectorReport | null;
}

export interface SceneRpfMount {
  archive: string;
  nested: string[];
  keys: string;
}

export interface SceneGameIndexSource {
  gameRoot: string;
  index: string;
  keys: string;
}

export interface SceneManifestReport {
  schemaVersion: number;
  root: {
    path: string;
    nameHash: string | null;
  };
  nodes: SceneNodeReport[];
  assets: SceneAssetReferenceReport[];
  summary: {
    totalEntities: number;
    emittedNodes: number;
    resolvedNodes: number;
    unresolvedNodes: number;
    assetReferences: number;
  };
  warnings: string[];
  limits: {
    maxNodes: number;
    truncated: boolean;
    omittedEntities: number;
  };
}


export interface WorkspaceExportPreflightReport {
  workspace: string;
  selectedRoots: string[];
  closureYmaps: string[];
  predictedFiles: string[];
  unresolved: {
    raw: number;
    vanilla: number;
    unknown: number;
    catalogUsed: boolean;
    durtyfreeUsed: boolean;
    fileCatalogUsed: boolean;
  };
  exportGate: {
    allowedWithoutOverride: boolean;
    requiresAllowUnresolved: boolean;
  };
  unknownGroups: Array<{
    kind: string;
    hash: string;
    uses: number;
    reasons: string[];
    affectedMaps: string[];
  }>;
  mloAudits: Array<{
    archetypeHash: string;
    ytyp: string;
    entities: number;
    uniqueEntityArchetypes: number;
    rooms: number;
    portals: number;
    localFiles: number;
    vanilla: number;
    unknown: number;
    risk: string;
  }>;
  warnings: string[];
}

export interface WorkspaceExportReport {
  workspace: string;
  resourceName: string;
  selectedRoots: string[];
  output: {
    resource: string;
    stream: string;
    manifest: string;
    metadata: string;
    gtxd: string | null;
    copiedFiles: string[];
    manifestMaps: number;
  };
  unresolved: {
    raw: number;
    vanilla: number;
    unknown: number;
    catalogUsed: boolean;
    durtyfreeUsed: boolean;
    fileCatalogUsed: boolean;
    allowUnresolved: boolean;
  };
  validation: {
    status: string;
    valid: boolean;
    metadataPresent: boolean;
    manifestValid: boolean;
    fxmanifestPresent: boolean;
    selectedRoots: number;
    closureMaps: number;
    copiedFiles: number;
    interiorMaps: number;
    interiorBounds: number;
    localMissing: string[];
    postExportRawUnresolved: number;
    postExportVanilla: number;
    postExportUnknown: number;
    warnings: string[];
    errors: string[];
  };
  warnings: string[];
}

export interface WorkspaceExportRequest {
  workspace: string;
  maps: string[];
  output: string;
  resourceName: string;
  allowUnresolved: boolean;
}

export interface OperationSpec {
  type: string;
  [key: string]: unknown;
}

export interface OperationDocument {
  schema: "ragelab.operation";
  schemaVersion: 1;
  source: string;
  output: string;
  operations: OperationSpec[];
}

export interface PlannedOperation {
  index: number;
  type: string;
  allowed: boolean;
  reason: string | null;
  details: unknown;
}

export interface OperationPlan {
  schema: "ragelab.operation.plan";
  schemaVersion: number;
  source: string;
  output: string;
  assetType: string;
  allowed: boolean;
  nonDestructive: boolean;
  outputExists: boolean;
  sourceBytes: number;
  operations: PlannedOperation[];
  reasons: string[];
  writes: string[];
}

export interface OperationApplyResult {
  schema: "ragelab.operation.apply";
  schemaVersion: number;
  source: string;
  output: string;
  assetType: string;
  operationsApplied: number;
  bytesWritten: number;
  nonDestructive: boolean;
  validation: {
    semanticReopen: boolean;
    sourceUnchanged: boolean;
  };
  details: unknown;
}

export function getStudioInfo(): Promise<StudioInfo> {
  return invoke<StudioInfo>("studio_info");
}

export function discoverGtaLegacy(): Promise<GtaVDiscoveryReport> {
  return invoke<GtaVDiscoveryReport>("core_discover_gta_legacy");
}

export function discoverFiveMLegacy(): Promise<FiveMDiscoveryReport> {
  return invoke<FiveMDiscoveryReport>("core_discover_fivem_legacy");
}

export function prepareGtaRpfKeys(
  gameRoot: string,
): Promise<GtaRpfKeyCacheReport> {
  return invoke<GtaRpfKeyCacheReport>("core_prepare_gta_rpf_keys", {
    request: {
      gameRoot,
    },
  });
}

export function prepareGtaRpfIndex(
  gameRoot: string,
  keys: string,
): Promise<GtaRpfIndexCacheReport> {
  return invoke<GtaRpfIndexCacheReport>("core_prepare_gta_rpf_index", {
    request: {
      gameRoot,
      keys,
    },
  });
}

export function inspectAsset(path: string): Promise<AssetInspectionReport> {
  return invoke<AssetInspectionReport>("core_asset_inspect", { path });
}

export function getAssetCapabilities(
  path: string,
): Promise<AssetCapabilitiesReport> {
  return invoke<AssetCapabilitiesReport>("core_asset_capabilities", { path });
}

export function previewAsset(
  path: string,
  options: CorePreviewOptions = {},
): Promise<AssetPreviewReport> {
  return invoke<AssetPreviewReport>("core_asset_preview", {
    request: {
      path,
      ...options,
    },
  });
}

export function assembleWorkspaceScene(
  workspace: string,
  ymap: string,
  maxNodes?: number,
  fallbackRoots: string[] = [],
  rpfMounts: SceneRpfMount[] = [],
  gameIndex: SceneGameIndexSource | null = null,
): Promise<SceneManifestReport> {
  return invoke<SceneManifestReport>("core_workspace_scene", {
    request: {
      workspace,
      ymap,
      fallbackRoots,
      rpfMounts,
      gameIndex,
      maxNodes,
    },
  });
}

export function previewWorkspaceSceneAsset(
  workspace: string,
  ymap: string,
  assetRef: number,
  maxNodes?: number,
  fallbackRoots: string[] = [],
  rpfMounts: SceneRpfMount[] = [],
  gameIndex: SceneGameIndexSource | null = null,
): Promise<AssetPreviewReport> {
  return invoke<AssetPreviewReport>("core_workspace_scene_asset_preview", {
    request: {
      workspace,
      ymap,
      assetRef,
      fallbackRoots,
      rpfMounts,
      gameIndex,
      maxNodes,
    },
  });
}

export function preflightWorkspaceExport(
  workspace: string,
  maps: string[],
): Promise<WorkspaceExportPreflightReport> {
  return invoke<WorkspaceExportPreflightReport>(
    "core_workspace_export_preflight",
    {
      request: {
        workspace,
        maps,
      },
    },
  );
}

export function exportWorkspace(
  request: WorkspaceExportRequest,
): Promise<WorkspaceExportReport> {
  return invoke<WorkspaceExportReport>("core_workspace_export", { request });
}

export function planOperation(
  document: OperationDocument,
): Promise<OperationPlan> {
  return invoke<OperationPlan>("core_operation_plan", { document });
}

export function applyOperation(
  document: OperationDocument,
): Promise<OperationApplyResult> {
  return invoke<OperationApplyResult>("core_operation_apply", { document });
}

export async function chooseWorkspaceDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open workspace",
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseSceneFallbackDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open optional loose asset fallback",
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseSceneRpfArchive(): Promise<string | null> {
  const selected = await open({
    directory: false,
    multiple: false,
    title: "Open read-only GTA RPF archive",
    filters: [
      {
        name: "RPF",
        extensions: ["rpf"],
      },
    ],
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseSceneRpfKeysDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open GTA Legacy RPF keys directory",
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseAssetFile(): Promise<string | null> {
  const selected = await open({
    directory: false,
    multiple: false,
    title: "Open RAGE asset",
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseWorkspaceYmaps(
  workspace: string,
): Promise<string[]> {
  const selected = await open({
    directory: false,
    multiple: true,
    title: "Select workspace YMAPs",
    defaultPath: workspace,
    filters: [
      {
        name: "YMAP",
        extensions: ["ymap"],
      },
    ],
  });

  if (Array.isArray(selected)) {
    return selected;
  }
  return typeof selected === "string" ? [selected] : [];
}

export async function chooseWorkspaceYmap(
  workspace: string,
): Promise<string | null> {
  const selected = await open({
    directory: false,
    multiple: false,
    title: "Open workspace YMAP",
    defaultPath: workspace,
    filters: [
      {
        name: "YMAP",
        extensions: ["ymap"],
      },
    ],
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseExportParentDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Choose export parent directory (outside workspace)",
  });

  return typeof selected === "string" ? selected : null;
}

export function buildExportOutputPath(
  parent: string,
  resourceName: string,
): Promise<string> {
  return join(parent, resourceName);
}

export async function chooseReplacementFile(
  title = "Select replacement payload",
): Promise<string | null> {
  const selected = await open({
    directory: false,
    multiple: false,
    title,
  });

  return typeof selected === "string" ? selected : null;
}

export async function chooseOperationOutput(
  defaultPath?: string,
): Promise<string | null> {
  const selected = await save({
    title: "Choose non-destructive output",
    defaultPath,
  });

  return typeof selected === "string" ? selected : null;
}
