<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

  import type { AssetPreviewReport } from "$lib/native";
  import {
    activeTruncations,
    previewViewModel,
    type CollisionPreviewPayload,
    type CollisionShape,
    type ModelPreviewPayload,
    type Vec3,
  } from "$lib/preview";

  export let report: AssetPreviewReport | null = null;

  let container: HTMLDivElement;
  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene | null = null;
  let camera: THREE.PerspectiveCamera | null = null;
  let controls: OrbitControls | null = null;
  let assetGroup: THREE.Group | null = null;
  let grid: THREE.GridHelper | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let animationFrame = 0;
  let mounted = false;
  let renderedReport: AssetPreviewReport | null = null;

  let renderError = "";
  let truncations: string[] = [];
  let visiblePrimitiveCount = 0;

  onMount(() => {
    try {
      initializeRenderer();
      mounted = true;
      renderCurrentReport();
      animate();
    } catch (error) {
      renderError =
        error instanceof Error ? error.message : "Unable to initialize preview renderer.";
    }

    return cleanup;
  });

  $: if (mounted && report !== renderedReport) {
    renderedReport = report;
    renderCurrentReport();
  }

  function initializeRenderer() {
    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0b0d10);

    camera = new THREE.PerspectiveCamera(45, 1, 0.01, 100000);
    camera.up.set(0, 0, 1);

    renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: false,
      powerPreference: "high-performance",
    });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    container.appendChild(renderer.domElement);

    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.screenSpacePanning = true;

    assetGroup = new THREE.Group();
    assetGroup.name = "asset-preview";
    scene.add(assetGroup);

    grid = new THREE.GridHelper(10, 10, 0x343a43, 0x242932);
    grid.rotation.x = Math.PI / 2;
    scene.add(grid);

    const ambient = new THREE.HemisphereLight(0xffffff, 0x232831, 1.35);
    scene.add(ambient);

    const key = new THREE.DirectionalLight(0xffffff, 1.5);
    key.position.set(4, -5, 7);
    scene.add(key);

    resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(container);
    resize();
  }

  function renderCurrentReport() {
    if (!assetGroup || !camera || !controls) {
      return;
    }

    clearGroup(assetGroup);
    renderError = "";
    truncations = [];
    visiblePrimitiveCount = 0;

    if (!report) {
      return;
    }

    const viewModel = previewViewModel(report);
    if (!viewModel) {
      renderError = "The core returned a preview payload this viewer does not recognize.";
      return;
    }

    truncations = activeTruncations(viewModel.payload.truncated);

    if (viewModel.kind === "model") {
      buildModelPreview(viewModel.payload, assetGroup);
    } else {
      buildCollisionPreview(viewModel.payload, assetGroup);
    }

    fitCamera(assetGroup);
  }

  function buildModelPreview(payload: ModelPreviewPayload, target: THREE.Group) {
    for (const primitive of payload.primitives) {
      if (!primitive.geometryIncluded || !primitive.geometry) {
        continue;
      }

      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute(
        "position",
        new THREE.Float32BufferAttribute(flattenVec3(primitive.geometry.positions), 3),
      );
      geometry.setIndex(primitive.geometry.indices);

      if (
        primitive.geometry.normals &&
        primitive.geometry.normals.length === primitive.geometry.positions.length
      ) {
        geometry.setAttribute(
          "normal",
          new THREE.Float32BufferAttribute(flattenVec3(primitive.geometry.normals), 3),
        );
      } else {
        geometry.computeVertexNormals();
      }

      geometry.computeBoundingSphere();

      const material = new THREE.MeshStandardMaterial({
        color: 0xbfc6cf,
        roughness: 0.82,
        metalness: 0.04,
        side: THREE.DoubleSide,
      });

      const mesh = new THREE.Mesh(geometry, material);
      mesh.name =
        "model-" + primitive.modelIndex + "-geometry-" + primitive.geometryIndex;
      target.add(mesh);
      visiblePrimitiveCount += 1;
    }
  }

  function buildCollisionPreview(
    payload: CollisionPreviewPayload,
    target: THREE.Group,
  ) {
    if (payload.mesh.positionsIncluded && payload.mesh.positions) {
      const positions = flattenVec3(payload.mesh.positions);

      for (const primitive of payload.mesh.primitives) {
        if (!primitive.indicesIncluded || !primitive.indices) {
          continue;
        }

        const geometry = new THREE.BufferGeometry();
        geometry.setAttribute(
          "position",
          new THREE.Float32BufferAttribute(positions, 3),
        );
        geometry.setIndex(primitive.indices);
        geometry.computeVertexNormals();

        const material = new THREE.MeshStandardMaterial({
          color: 0x78a8ca,
          roughness: 0.95,
          metalness: 0,
          transparent: true,
          opacity: 0.34,
          wireframe: true,
          side: THREE.DoubleSide,
        });

        const mesh = new THREE.Mesh(geometry, material);
        mesh.name = "collision-mesh-" + primitive.childIndex;
        target.add(mesh);
        visiblePrimitiveCount += 1;
      }
    }

    for (const primitive of payload.shapePrimitives) {
      const shapeObject = buildCollisionShape(primitive.shape);
      shapeObject.name =
        "collision-shape-" + primitive.childIndex + "-" + primitive.polygonIndex;
      target.add(shapeObject);
      visiblePrimitiveCount += 1;
    }
  }

  function buildCollisionShape(shape: CollisionShape): THREE.Object3D {
    switch (shape.kind) {
      case "sphere":
        return sphereObject(shape.center, shape.radius);
      case "capsule": {
        const group = new THREE.Group();
        group.add(segmentCylinder(shape.start, shape.end, shape.radius));
        group.add(sphereObject(shape.start, shape.radius));
        group.add(sphereObject(shape.end, shape.radius));
        return group;
      }
      case "cylinder":
        return segmentCylinder(shape.start, shape.end, shape.radius);
      case "box":
        return boxWireframe(shape.corner, shape.edges);
    }
  }

  function sphereObject(center: Vec3, radius: number): THREE.Mesh {
    const geometry = new THREE.SphereGeometry(Math.max(radius, 0.0001), 18, 12);
    const material = collisionShapeMaterial();
    const mesh = new THREE.Mesh(geometry, material);
    mesh.position.fromArray(center);
    return mesh;
  }

  function segmentCylinder(start: Vec3, end: Vec3, radius: number): THREE.Mesh {
    const startVector = new THREE.Vector3().fromArray(start);
    const endVector = new THREE.Vector3().fromArray(end);
    const direction = endVector.clone().sub(startVector);
    const length = Math.max(direction.length(), 0.0001);

    const geometry = new THREE.CylinderGeometry(
      Math.max(radius, 0.0001),
      Math.max(radius, 0.0001),
      length,
      16,
      1,
      true,
    );
    const material = collisionShapeMaterial();
    const mesh = new THREE.Mesh(geometry, material);

    mesh.position.copy(startVector).add(endVector).multiplyScalar(0.5);

    if (direction.lengthSq() > 0) {
      mesh.quaternion.setFromUnitVectors(
        new THREE.Vector3(0, 1, 0),
        direction.normalize(),
      );
    }

    return mesh;
  }

  function boxWireframe(corner: Vec3, edges: [Vec3, Vec3, Vec3]): THREE.LineSegments {
    const c = new THREE.Vector3().fromArray(corner);
    const a = new THREE.Vector3().fromArray(edges[0]);
    const b = new THREE.Vector3().fromArray(edges[1]);
    const d = new THREE.Vector3().fromArray(edges[2]);

    const vertices = [
      c.clone(),
      c.clone().add(a),
      c.clone().add(b),
      c.clone().add(d),
      c.clone().add(a).add(b),
      c.clone().add(a).add(d),
      c.clone().add(b).add(d),
      c.clone().add(a).add(b).add(d),
    ];

    const edgePairs = [
      [0, 1],
      [0, 2],
      [0, 3],
      [1, 4],
      [1, 5],
      [2, 4],
      [2, 6],
      [3, 5],
      [3, 6],
      [4, 7],
      [5, 7],
      [6, 7],
    ] as const;

    const points: THREE.Vector3[] = [];
    for (const [left, right] of edgePairs) {
      points.push(vertices[left], vertices[right]);
    }

    const geometry = new THREE.BufferGeometry().setFromPoints(points);
    const material = new THREE.LineBasicMaterial({ color: 0xd5b77a });
    return new THREE.LineSegments(geometry, material);
  }

  function collisionShapeMaterial(): THREE.MeshStandardMaterial {
    return new THREE.MeshStandardMaterial({
      color: 0xd5b77a,
      roughness: 0.85,
      metalness: 0,
      transparent: true,
      opacity: 0.5,
      wireframe: true,
    });
  }

  function fitCamera(target: THREE.Object3D) {
    if (!camera || !controls) {
      return;
    }

    const bounds = new THREE.Box3().setFromObject(target);
    if (bounds.isEmpty()) {
      controls.target.set(0, 0, 0);
      camera.position.set(3, -3, 2.5);
      controls.update();
      return;
    }

    const sphere = bounds.getBoundingSphere(new THREE.Sphere());
    const radius = Math.max(sphere.radius, 0.5);
    const distance = radius * 2.8;

    controls.target.copy(sphere.center);
    camera.position
      .copy(sphere.center)
      .add(new THREE.Vector3(distance, -distance, distance * 0.72));

    camera.near = Math.max(radius / 1000, 0.001);
    camera.far = Math.max(radius * 100, 100);
    camera.updateProjectionMatrix();

    if (grid) {
      grid.position.z = bounds.min.z;
      grid.scale.setScalar(Math.max(radius / 5, 0.1));
    }

    controls.update();
  }

  function flattenVec3(values: Vec3[]): number[] {
    const flattened: number[] = [];
    for (const value of values) {
      flattened.push(value[0], value[1], value[2]);
    }
    return flattened;
  }

  function clearGroup(group: THREE.Group) {
    const children = [...group.children];
    group.clear();

    for (const child of children) {
      child.traverse((object) => {
        const renderable = object as THREE.Mesh | THREE.LineSegments;
        if ("geometry" in renderable && renderable.geometry) {
          renderable.geometry.dispose();
        }

        if ("material" in renderable && renderable.material) {
          const materials = Array.isArray(renderable.material)
            ? renderable.material
            : [renderable.material];
          for (const material of materials) {
            material.dispose();
          }
        }
      });
    }
  }

  function resize() {
    if (!renderer || !camera || !container) {
      return;
    }

    const width = Math.max(container.clientWidth, 1);
    const height = Math.max(container.clientHeight, 1);
    renderer.setSize(width, height, false);
    camera.aspect = width / height;
    camera.updateProjectionMatrix();
  }

  function animate() {
    controls?.update();
    if (renderer && scene && camera) {
      renderer.render(scene, camera);
    }
    animationFrame = window.requestAnimationFrame(animate);
  }

  function cleanup() {
    mounted = false;

    if (animationFrame) {
      window.cancelAnimationFrame(animationFrame);
      animationFrame = 0;
    }

    resizeObserver?.disconnect();
    resizeObserver = null;

    controls?.dispose();
    controls = null;

    if (assetGroup) {
      clearGroup(assetGroup);
    }

    grid?.geometry.dispose();
    if (Array.isArray(grid?.material)) {
      for (const material of grid.material) {
        material.dispose();
      }
    } else {
      grid?.material.dispose();
    }

    renderer?.dispose();
    renderer?.domElement.remove();

    renderer = null;
    scene = null;
    camera = null;
    assetGroup = null;
    grid = null;
  }
</script>

<section class="preview-shell">
  <div class="preview-meta">
    <div>
      <p class="label">Visual preview</p>
      <div class="meta-line">
        <span>{report?.type ?? "No asset"}</span>
        {#if report}
          <span>{report.spatial.classification}</span>
          <span>{report.spatial.coordinateConvention}</span>
          <span>{visiblePrimitiveCount} rendered primitive(s)</span>
        {/if}
      </div>
    </div>

    <span class="hint">Drag to orbit · wheel to zoom · right-drag to pan</span>
  </div>

  {#if truncations.length > 0}
    <div class="truncation">
      <strong>Preview is bounded by the core</strong>
      <span>{truncations.join(" · ")}</span>
    </div>
  {/if}

  {#if renderError}
    <div class="render-error" role="alert">{renderError}</div>
  {/if}

  <div class="viewport" bind:this={container}></div>
</section>

<style>
  .preview-shell {
    margin-top: 14px;
    border: 1px solid #292e35;
    border-radius: 10px;
    overflow: hidden;
    background: #0b0d10;
  }

  .preview-meta {
    min-height: 58px;
    padding: 12px 14px;
    border-bottom: 1px solid #292e35;
    background: #12151a;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 20px;
  }

  .label {
    margin: 0 0 5px;
    color: #7d838c;
    font-size: 11px;
    font-weight: 650;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .meta-line {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .meta-line span {
    padding: 3px 6px;
    border: 1px solid #2d3239;
    border-radius: 5px;
    color: #9299a3;
    font-size: 10px;
  }

  .hint {
    color: #69707a;
    font-size: 10px;
    text-align: right;
  }

  .viewport {
    width: 100%;
    height: min(58vh, 560px);
    min-height: 340px;
  }

  .viewport :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
  }

  .truncation,
  .render-error {
    padding: 10px 14px;
    border-bottom: 1px solid #3a3223;
    background: #1d1912;
    color: #c4ab7d;
    font-size: 11px;
  }

  .truncation strong {
    margin-right: 8px;
  }

  .render-error {
    border-color: #493027;
    background: #1c1411;
    color: #d2a187;
  }

  @media (max-width: 700px) {
    .preview-meta {
      display: grid;
    }

    .hint {
      text-align: left;
    }

    .viewport {
      height: 360px;
      min-height: 300px;
    }
  }
</style>
