import {
  previewWorkspaceSceneAsset,
  type AssetPreviewReport,
  type SceneAssetReferenceReport,
  type SceneGameIndexSource,
  type SceneManifestReport,
  type SceneRpfMount,
} from "$lib/native";

export const SCENE_PREVIEW_ASSET_LIMIT = 24;
export const SCENE_PREVIEW_CONCURRENCY = 3;

export interface ScenePreviewContext {
  workspace: string;
  ymap: string;
  maxNodes?: number;
  fallbackRoots?: string[];
  rpfMounts?: SceneRpfMount[];
  gameIndex?: SceneGameIndexSource | null;
}

export interface ScenePreviewLoadResult {
  previews: Record<number, AssetPreviewReport>;
  errors: Record<number, string>;
  eligibleAssets: number;
  requestedAssets: number;
  reusedNodeReferences: number;
  omittedAssets: number;
  skippedWithoutScale: number;
}

export async function loadSceneAssetPreviews(
  manifest: SceneManifestReport,
  context: ScenePreviewContext,
): Promise<ScenePreviewLoadResult> {
  const assetById = new Map(
    manifest.assets.map((asset) => [asset.id, asset] as const),
  );
  const requestedIds: number[] = [];
  const seen = new Set<number>();
  let skippedWithoutScale = 0;
  let reusedNodeReferences = 0;

  for (const node of manifest.nodes) {
    if (
      node.resolution !== "resolved" ||
      node.assetRef === null ||
      !node.transform
    ) {
      continue;
    }

    if (!node.transform.scale) {
      skippedWithoutScale += 1;
      continue;
    }

    const asset = assetById.get(node.assetRef);
    if (!asset || !isPreviewableModelAsset(asset)) {
      continue;
    }

    if (seen.has(asset.id)) {
      reusedNodeReferences += 1;
      continue;
    }

    seen.add(asset.id);
    requestedIds.push(asset.id);
  }

  const eligibleAssets = requestedIds.length;
  const selectedIds = requestedIds.slice(0, SCENE_PREVIEW_ASSET_LIMIT);
  const omittedAssets = Math.max(
    0,
    eligibleAssets - SCENE_PREVIEW_ASSET_LIMIT,
  );

  const previews: Record<number, AssetPreviewReport> = {};
  const errors: Record<number, string> = {};
  let requestedAssets = 0;
  let cursor = 0;

  async function worker() {
    while (cursor < selectedIds.length) {
      const index = cursor++;
      const assetId = selectedIds[index];
      const asset = assetById.get(assetId);
      if (!asset) {
        continue;
      }

      requestedAssets += 1;

      try {
        previews[asset.id] = await previewWorkspaceSceneAsset(
          context.workspace,
          context.ymap,
          asset.id,
          context.maxNodes,
          context.fallbackRoots ?? [],
          context.rpfMounts ?? [],
          context.gameIndex ?? null,
        );
      } catch (error) {
        errors[asset.id] =
          error instanceof Error && error.message
            ? error.message
            : "RageLab core could not build this scene asset preview.";
      }
    }
  }

  const workerCount = Math.min(
    SCENE_PREVIEW_CONCURRENCY,
    Math.max(selectedIds.length, 1),
  );
  await Promise.all(Array.from({ length: workerCount }, () => worker()));

  return {
    previews,
    errors,
    eligibleAssets,
    requestedAssets,
    reusedNodeReferences,
    omittedAssets,
    skippedWithoutScale,
  };
}

function isPreviewableModelAsset(asset: SceneAssetReferenceReport): boolean {
  return (
    asset.kind === "YDR" || asset.kind === "YDD" || asset.kind === "YFT"
  );
}
