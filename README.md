# RageLab Studio

RageLab Studio is the desktop visual interface for [RageLab](https://github.com/RageLab/ragelab).

It provides native workspace access, visual inspection, scene rendering, editing workflows, and export UX while keeping RAGE parsing, validation, dependency resolution, and writer safety in the Rust core.

## Architecture

RageLab Studio is a client of RageLab, not a second implementation of RAGE formats.

```text
RageLab Studio
  Tauri
  Svelte
  native wgpu viewport
  Three.js fallback
      |
      | typed native commands / RenderPackage
      v
RageLab
  Rust core
  workspace/resolver
  format readers and writers
```

Frontend code is responsible for presentation and interaction. Binary format knowledge and write eligibility remain in Rust.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Current status

The repository currently provides the desktop application shell, native workspace directory selection, GTA V Legacy/FiveM discovery, direct engine-backed asset inspection/capability discovery, plan-first declarative Legacy editing, an engine-backed YMAP workspace scene browser, and typed native bridges for workspace export preflight/export reports. The scene browser uses the reusable RageLab `ragelab-render` wgpu surface renderer by default: one shared `RenderPackage` carries resolved geometry, materials, textures, provenance and transforms from Core into GPU caches without reparsing RAGE data in the UI. The previous Three.js preview viewport remains available as a lazy-loaded fallback. Editing always chooses a separate output path, runs a dry-run plan first, invalidates that plan when parameters change, and delegates apply/replan/semantic verification to the Rust core.

The Rust adapter and native renderer are reproducibly pinned to RageLab core revision `63f5fec65c195e318c3477da0888e5cabee53bfb`. Studio builds do not depend on a local RageLab checkout.

## Development

Requirements:

- Rust toolchain
- Bun
- Windows, macOS, or Linux dependencies required by Tauri 2

Install dependencies:

```bash
bun install
```

Run frontend checks:

```bash
bun run check
bun run build
```

Run the desktop application:

```bash
bun run tauri dev
```

## Project boundaries

- No RAGE binary parsing in TypeScript.
- No writer safety decisions in the frontend.
- No dependency-resolution logic duplicated from RageLab.
- Source assets are not overwritten implicitly.
- Native filesystem access is exposed through narrow desktop commands.
- Asset inspection, capabilities, preview bounds, GTA/FiveM discovery, YMAP workspace scene assembly, provider resolution, scene transforms/collision state, workspace export preflight/export, operation planning, and mutation are delegated directly to `ragelab-engine`.
- Studio export uses core reports, `overwrite=false`, an absolute create-new output path outside the source workspace, and no adapter-side dependency resolution.
- The native scene viewport consumes one Core-owned `RenderPackage`; scene geometry/material/texture semantics are not reconstructed in TypeScript.
- Three.js remains an explicit fallback for diagnostics/recovery. Its preview path is loaded only when selected, is UI-bounded to 96 unique assets with concurrency 3, and caps real preview instances at 512 nodes.
- YBN collision dependencies remain `localOnly` and are not world-placed because the current SceneManifest does not provide an explicit collision placement transform.
- Declarative edit forms are presentation/schema mapping only; a successful core plan is required before apply.
- Studio operation source/output paths must be absolute, and output must differ from source; existing outputs remain rejected by the core create-new policy.
- Core revisions are pinned by public Git commit; committed local path dependencies are not permitted.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

RageLab is an independent open-source project and is not affiliated with or endorsed by Rockstar Games or Take-Two Interactive.
