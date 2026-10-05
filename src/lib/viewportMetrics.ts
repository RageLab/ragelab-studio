export interface ThreeViewportMetrics {
  sceneBuildMs: number;
  averageFrameMs: number;
  sampledFrames: number;
  rendererGeometries: number;
  rendererTextures: number;
  sharedGeometryAssets: number;
  sharedDiffuseTextures: number;
  texturedMaterials: number;
  realGeometryNodes: number;
  fallbackProxyNodes: number;
  payloadBytes: number;
}

export const EMPTY_THREE_VIEWPORT_METRICS: ThreeViewportMetrics = {
  sceneBuildMs: 0,
  averageFrameMs: 0,
  sampledFrames: 0,
  rendererGeometries: 0,
  rendererTextures: 0,
  sharedGeometryAssets: 0,
  sharedDiffuseTextures: 0,
  texturedMaterials: 0,
  realGeometryNodes: 0,
  fallbackProxyNodes: 0,
  payloadBytes: 0,
};
