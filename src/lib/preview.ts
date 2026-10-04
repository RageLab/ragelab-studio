import type { AssetPreviewReport } from "$lib/native";

export type Vec2 = [number, number];
export type Vec3 = [number, number, number];

export interface ModelPreviewGeometry {
  positions: Vec3[];
  normals: Vec3[] | null;
  uv0: Vec2[] | null;
  indices: number[];
}

export interface ModelPreviewPrimitive {
  modelIndex: number;
  geometryIndex: number;
  shaderIndex: number | null;
  topology: string;
  geometryIncluded: boolean;
  geometryOmittedReason: string | null;
  geometry: ModelPreviewGeometry | null;
}

export interface ModelPreviewPayload {
  selector: Record<string, unknown> | null;
  name: string | null;
  lod: string;
  coordinateConvention: string;
  bounds: {
    center: Vec3;
    radius: number;
    min: Vec3;
    max: Vec3;
  };
  counts: Record<string, number>;
  primitives: ModelPreviewPrimitive[];
  truncated: Record<string, boolean | number>;
}

export type CollisionShape =
  | { kind: "sphere"; center: Vec3; radius: number }
  | { kind: "capsule"; start: Vec3; end: Vec3; radius: number }
  | { kind: "box"; corner: Vec3; edges: [Vec3, Vec3, Vec3] }
  | { kind: "cylinder"; start: Vec3; end: Vec3; radius: number };

export interface CollisionMeshPrimitive {
  childIndex: number;
  materialIndex: number | null;
  indexCount: number;
  triangleCount: number;
  indicesIncluded: boolean;
  indicesOmittedReason: string | null;
  indices: number[] | null;
}

export interface CollisionShapePrimitive {
  childIndex: number;
  materialIndex: number | null;
  polygonIndex: number;
  shape: CollisionShape;
}

export interface CollisionPreviewPayload {
  kind: "collision";
  coordinateConvention: string;
  bounds: {
    min: Vec3;
    max: Vec3;
    center: Vec3;
    sphereCenter: Vec3;
    sphereRadius: number;
  };
  counts: Record<string, number>;
  mesh: {
    positionsIncluded: boolean;
    positionsOmittedReason: string | null;
    positions: Vec3[] | null;
    primitives: CollisionMeshPrimitive[];
  };
  shapePrimitives: CollisionShapePrimitive[];
  truncated: Record<string, boolean | number>;
}

export type PreviewViewModel =
  | {
      kind: "model";
      report: AssetPreviewReport;
      payload: ModelPreviewPayload;
    }
  | {
      kind: "collision";
      report: AssetPreviewReport;
      payload: CollisionPreviewPayload;
    };

export function previewViewModel(
  report: AssetPreviewReport,
): PreviewViewModel | null {
  if (!isRecord(report.preview)) {
    return null;
  }

  if (
    report.type === "YBN" &&
    report.preview.kind === "collision" &&
    isRecord(report.preview.mesh) &&
    Array.isArray(report.preview.shapePrimitives)
  ) {
    return {
      kind: "collision",
      report,
      payload: report.preview as unknown as CollisionPreviewPayload,
    };
  }

  if (
    (report.type === "YDR" ||
      report.type === "YDD" ||
      report.type === "YFT") &&
    Array.isArray(report.preview.primitives) &&
    isRecord(report.preview.bounds)
  ) {
    return {
      kind: "model",
      report,
      payload: report.preview as unknown as ModelPreviewPayload,
    };
  }

  return null;
}

export function activeTruncations(
  truncated: Record<string, boolean | number>,
): string[] {
  return Object.entries(truncated)
    .filter(([, value]) => value === true || (typeof value === "number" && value > 0))
    .map(([key, value]) => (value === true ? key : key + ": " + value));
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}
