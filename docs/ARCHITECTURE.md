# Architecture

RageLab Studio is a desktop adapter over the RageLab Rust core.

## Ownership

### RageLab

The core repository owns:

- binary container and format parsing;
- binary writers and write eligibility;
- workspace indexing;
- dependency resolution;
- export policy and validation;
- machine-readable capabilities and operation contracts;
- asset type detection, inspection, validation, and per-file capabilities;
- bounded renderer-neutral YDR/YDD/YBN preview policy;
- GTA V Legacy and FiveM environment discovery;
- YMAP workspace scene assembly and serialized `SceneManifestReport` contracts;
- workspace export preflight/export policy and serialized `WorkspaceExportPreflightReport` / `WorkspaceExportReport` contracts.

### RageLab Studio

This repository owns:

- desktop lifecycle through Tauri;
- local workspace selection and desktop permissions;
- Svelte application state and interaction;
- native Tauri window/surface lifecycle for `ragelab-render`;
- Three.js fallback rendering;
- editor presentation, comparison, and recovery UX;
- native command adapters required by desktop workflows.

## Invariant

A RAGE operation is not complete if it can only be executed through RageLab Studio.

Reusable behavior must be implemented in RageLab first and then exposed to the desktop application. The Studio may explain a core capability or rejection reason, but it must not recreate binary-layout or writer-safety decisions.

## Native boundary

The frontend communicates with Rust through typed Tauri commands.

```text
Svelte
  | invoke / normalized input / physical viewport rect
  v
Tauri commands --------------------------+
  |                                      |
  | scene/report APIs                    | native Window (no WebView)
  v                                      v
RageLab engine -> RenderPackage -> ragelab-render / wgpu
                          |
                          +-> Three.js fallback uses bounded preview reports
```

Filesystem access, GTA/FiveM discovery, asset inspection, capability discovery, bounded preview construction, workspace indexing, YMAP scene assembly, workspace export preflight/export, mutation planning, and write operations belong on the native side of this boundary. Tauri commands should be thin wrappers that serialize core reports; they must not reinterpret format metadata, scene resolution states/transforms, export dependency/gate policy, writer eligibility, preview hard caps, or discovery evidence. Studio export additionally requires an absolute create-new output path outside the source workspace and never enables overwrite.

The native scene viewport is responsible only for presenting the renderer-neutral `RenderPackage` produced by RageLab. The Tauri adapter owns window placement and normalized input forwarding; it does not parse assets or reinterpret provider/material/transform semantics. `ragelab-render` owns GPU caches, camera state, picking and highlight state. The Three.js viewport remains a fallback that visualizes bounded core reports; it may choose camera, lighting, materials, wireframes, and interaction behavior, but it must not reopen assets, derive writer eligibility, expand omitted geometry, or reinterpret a truncated/local-only preview as complete world-space data.

For `SceneManifestReport`, Studio may render symbolic node proxies and selection affordances using the returned translation/quaternion and explicit scale when present. Proxy dimensions are presentation glyphs, not asset bounds. When core scale is absent, Studio must not report or infer identity scale. Provider resolution, unresolved reason codes, collision relationship state, truncation, and node limits remain authoritative core data.

The native viewport loads one shared `RenderPackage` and therefore does not fan out per-asset JSON preview requests. GPU meshes, materials and textures are cached by stable Core identity; read-only game instances are drawn first and loose workspace instances are drawn last with a subtle workspace tint. When the Three.js fallback is selected, resolved visual assets are loaded lazily through the bounded `AssetPreviewReport` service. That fallback deduplicates by scene asset reference, limits a scene to 96 unique preview assets, uses at most 3 concurrent preview requests, and caps real geometry placement at 512 nodes. These are fallback presentation/performance budgets and do not replace Core renderer/package limits.

Collision remains stricter. A `SceneCollisionRelationship` marked `localOnly` proves a dependency, not a world placement transform. Studio may render a relationship marker, but it must not place YBN geometry in the world until the core provides explicit collision placement evidence.

Declarative editing follows the same rule. The frontend may map known operation schema fields to controls and assemble a versioned `OperationDocument`, but it exposes only writer IDs reported by per-file capabilities. Tauri requires absolute source/output paths, a separate output path, and absolute external replacement payload paths. The frontend invalidates a plan after any operation/output change and enables apply only for the exact document that received `allowed: true`. `ragelab-engine::apply_operation_document` still replans immediately before mutation and retains create-new output, semantic-reopen, and source-unchanged verification as the final authority.

## Repository independence

RageLab Studio must build from its own repository. It must not depend on local paths to development clones or private workspaces. The committed core dependency uses the public RageLab Git repository pinned to an exact commit revision; `StudioInfo.coreRevision` exposes that revision to the desktop shell. Development against unreleased RageLab code may use a documented local override, but that override must never be committed.
