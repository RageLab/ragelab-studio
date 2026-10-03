<script lang="ts">
  import { onMount } from "svelte";
  import * as THREE from "three";
  import { OrbitControls } from "three/examples/jsm/controls/OrbitControls.js";

  import type { SceneManifestReport, SceneNodeReport } from "$lib/native";

  export let manifest: SceneManifestReport | null = null;
  export let selectedNodeIndex: number | null = null;

  let container: HTMLDivElement;
  let renderer: THREE.WebGLRenderer | null = null;
  let scene: THREE.Scene | null = null;
  let camera: THREE.PerspectiveCamera | null = null;
  let controls: OrbitControls | null = null;
  let proxyGroup: THREE.Group | null = null;
  let grid: THREE.GridHelper | null = null;
  let resizeObserver: ResizeObserver | null = null;
  let animationFrame = 0;
  let mounted = false;
  let renderedManifest: SceneManifestReport | null = null;
  let previousSelectedNodeIndex: number | null = null;

  let renderError = "";
  let placedNodes = 0;
  let unplacedNodes = 0;

  onMount(() => {
    try {
      initializeRenderer();
      mounted = true;
      renderCurrentManifest();
      animate();
    } catch (error) {
      renderError =
        error instanceof Error
          ? error.message
          : "Unable to initialize the scene renderer.";
    }

    return cleanup;
  });

  $: if (mounted && manifest !== renderedManifest) {
    renderedManifest = manifest;
    renderCurrentManifest();
  }

  $: if (mounted && selectedNodeIndex !== previousSelectedNodeIndex) {
    previousSelectedNodeIndex = selectedNodeIndex;
    updateSelectionStyles();
  }

  function initializeRenderer() {
    scene = new THREE.Scene();
    scene.background = new THREE.Color(0x0a0c0f);

    camera = new THREE.PerspectiveCamera(45, 1, 0.01, 1_000_000);
    camera.up.set(0, 0, 1);

    renderer = new THREE.WebGLRenderer({
      antialias: true,
      alpha: false,
      powerPreference: "high-performance",
    });
    renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.domElement.addEventListener("pointerdown", selectProxy);
    container.appendChild(renderer.domElement);

    controls = new OrbitControls(camera, renderer.domElement);
    controls.enableDamping = true;
    controls.dampingFactor = 0.08;
    controls.screenSpacePanning = true;

    proxyGroup = new THREE.Group();
    proxyGroup.name = "scene-node-proxies";
    scene.add(proxyGroup);

    grid = new THREE.GridHelper(100, 20, 0x353b44, 0x22272f);
    grid.rotation.x = Math.PI / 2;
    scene.add(grid);

    const ambient = new THREE.HemisphereLight(0xffffff, 0x222831, 1.35);
    scene.add(ambient);

    const key = new THREE.DirectionalLight(0xffffff, 1.2);
    key.position.set(6, -8, 10);
    scene.add(key);

    resizeObserver = new ResizeObserver(resize);
    resizeObserver.observe(container);
    resize();
  }

  function renderCurrentManifest() {
    if (!proxyGroup || !camera || !controls) {
      return;
    }

    clearGroup(proxyGroup);
    renderError = "";
    placedNodes = 0;
    unplacedNodes = 0;

    if (!manifest) {
      return;
    }

    for (const node of manifest.nodes) {
      if (!node.transform) {
        unplacedNodes += 1;
        continue;
      }

      const proxy = buildNodeProxy(node);
      proxyGroup.add(proxy);
      placedNodes += 1;
    }

    updateSelectionStyles();
    fitCamera(proxyGroup);
  }

  function buildNodeProxy(node: SceneNodeReport): THREE.Group {
    const transform = node.transform;
    if (!transform) {
      throw new Error(
        "Scene proxy rendering requires a core-provided world transform.",
      );
    }

    const group = new THREE.Group();
    group.name = "scene-node-" + node.index;
    group.userData.nodeIndex = node.index;
    group.userData.selectable = true;

    const geometry =
      node.assetKind === "YDD"
        ? new THREE.OctahedronGeometry(0.65, 0)
        : new THREE.BoxGeometry(1.2, 1.2, 1.2);

    const material = new THREE.MeshStandardMaterial({
      color: node.resolution === "resolved" ? 0x7f9db8 : 0xa46f68,
      roughness: 0.82,
      metalness: 0.02,
      transparent: true,
      opacity: node.resolution === "resolved" ? 0.86 : 0.62,
    });

    const mesh = new THREE.Mesh(geometry, material);
    mesh.userData.nodeIndex = node.index;
    mesh.userData.selectable = true;
    group.add(mesh);

    if (node.resolution === "unresolved") {
      const outline = new THREE.LineSegments(
        new THREE.EdgesGeometry(geometry),
        new THREE.LineBasicMaterial({ color: 0xd18c82 }),
      );
      group.add(outline);
    }

    if (node.collision) {
      const ringGeometry = new THREE.RingGeometry(0.78, 0.92, 24);
      const ringMaterial = new THREE.MeshBasicMaterial({
        color:
          node.collision.state === "localOnly" ? 0xd3ad69 : 0xb16f6f,
        side: THREE.DoubleSide,
        transparent: true,
        opacity: 0.8,
      });
      const ring = new THREE.Mesh(ringGeometry, ringMaterial);
      ring.position.z = -0.72;
      group.add(ring);
    }

    group.position.fromArray(transform.translation);
    group.quaternion.fromArray(transform.rotation);

    if (transform.scale) {
      group.scale.fromArray(transform.scale);
      group.userData.proxyScaleSource = "core";
    } else {
      group.userData.proxyScaleSource = "symbolic";
    }

    return group;
  }

  function selectProxy(event: PointerEvent) {
    if (!renderer || !camera || !proxyGroup) {
      return;
    }

    const rect = renderer.domElement.getBoundingClientRect();
    const pointer = new THREE.Vector2(
      ((event.clientX - rect.left) / rect.width) * 2 - 1,
      -((event.clientY - rect.top) / rect.height) * 2 + 1,
    );
    const raycaster = new THREE.Raycaster();
    raycaster.setFromCamera(pointer, camera);

    const hit = raycaster
      .intersectObjects(proxyGroup.children, true)
      .find((intersection) => selectableNodeIndex(intersection.object) !== null);

    if (!hit) {
      selectedNodeIndex = null;
      updateSelectionStyles();
      return;
    }

    selectedNodeIndex = selectableNodeIndex(hit.object);
    updateSelectionStyles();
  }

  function selectableNodeIndex(object: THREE.Object3D): number | null {
    let current: THREE.Object3D | null = object;
    while (current) {
      if (
        current.userData.selectable === true &&
        typeof current.userData.nodeIndex === "number"
      ) {
        return current.userData.nodeIndex;
      }
      current = current.parent;
    }
    return null;
  }

  function updateSelectionStyles() {
    if (!proxyGroup) {
      return;
    }

    proxyGroup.traverse((object) => {
      if (!(object instanceof THREE.Mesh)) {
        return;
      }

      const nodeIndex = selectableNodeIndex(object);
      if (nodeIndex === null || !(object.material instanceof THREE.MeshStandardMaterial)) {
        return;
      }

      if (nodeIndex === selectedNodeIndex) {
        object.material.emissive.setHex(0x4e6174);
        object.material.emissiveIntensity = 0.85;
      } else {
        object.material.emissive.setHex(0x000000);
        object.material.emissiveIntensity = 0;
      }
    });
  }

  function fitCamera(target: THREE.Object3D) {
    if (!camera || !controls) {
      return;
    }

    const bounds = new THREE.Box3().setFromObject(target);
    if (bounds.isEmpty()) {
      controls.target.set(0, 0, 0);
      camera.position.set(8, -8, 6);
      controls.update();
      return;
    }

    const sphere = bounds.getBoundingSphere(new THREE.Sphere());
    const radius = Math.max(sphere.radius, 1);
    const distance = radius * 2.6;

    controls.target.copy(sphere.center);
    camera.position
      .copy(sphere.center)
      .add(new THREE.Vector3(distance, -distance, distance * 0.7));

    camera.near = Math.max(radius / 5000, 0.01);
    camera.far = Math.max(radius * 200, 1000);
    camera.updateProjectionMatrix();

    if (grid) {
      grid.position.z = bounds.min.z;
      grid.scale.setScalar(Math.max(radius / 25, 0.25));
    }

    controls.update();
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

    if (renderer) {
      renderer.domElement.removeEventListener("pointerdown", selectProxy);
    }

    if (proxyGroup) {
      clearGroup(proxyGroup);
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
    proxyGroup = null;
    grid = null;
  }
</script>

<section class="scene-viewport">
  <div class="viewport-meta">
    <div>
      <p class="label">Scene proxies</p>
      <div class="meta-line">
        <span>{placedNodes} placed</span>
        <span>{unplacedNodes} without transform</span>
        {#if selectedNodeIndex !== null}
          <span>selected #{selectedNodeIndex}</span>
        {/if}
      </div>
    </div>

    <span class="hint">
      Click node · orbit/zoom/pan · proxy size is symbolic when core scale is absent
    </span>
  </div>

  {#if renderError}
    <div class="render-error" role="alert">{renderError}</div>
  {/if}

  <div class="viewport" bind:this={container}></div>
</section>

<style>
  .scene-viewport {
    border: 1px solid #292e35;
    border-radius: 10px;
    overflow: hidden;
    background: #0a0c0f;
  }

  .viewport-meta {
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
    min-height: 360px;
  }

  .viewport :global(canvas) {
    display: block;
    width: 100%;
    height: 100%;
  }

  .render-error {
    padding: 10px 14px;
    border-bottom: 1px solid #493027;
    background: #1c1411;
    color: #d2a187;
    font-size: 11px;
  }

  @media (max-width: 700px) {
    .viewport-meta {
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
