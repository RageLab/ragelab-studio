# RageLab Studio

RageLab Studio is the desktop visual interface for [RageLab](https://github.com/RageLab/ragelab).

It provides native workspace access, visual inspection, scene rendering, editing workflows, and export UX while keeping RAGE parsing, validation, dependency resolution, and writer safety in the Rust core.

## Architecture

RageLab Studio is a client of RageLab, not a second implementation of RAGE formats.

```text
RageLab Studio
  Tauri
  Svelte
  Three.js
      |
      | native commands
      v
RageLab
  Rust core
  workspace/resolver
  format readers and writers
```

Frontend code is responsible for presentation and interaction. Binary format knowledge and write eligibility remain in Rust.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md).

## Current status

The repository currently provides the desktop application shell, native workspace directory selection, GTA V Legacy/FiveM discovery, direct engine-backed asset inspection/capability discovery, bounded YDR/YDD/YBN preview, plan-first declarative Legacy editing, and an engine-backed YMAP workspace scene browser. The scene browser selects a YMAP inside the active workspace, renders core summary/warnings/limits and unresolved reasons, and upgrades eligible resolved YDR/YDD nodes from symbolic proxies to bounded core preview geometry placed with the exact SceneManifest translation/quaternion/scale. Editing always chooses a separate output path, runs a dry-run plan first, invalidates that plan when parameters change, and delegates apply/replan/semantic verification to the Rust core.

The Rust adapter is reproducibly pinned to RageLab core revision `ba5f7a64a89de3fb8e9e2acd59e0f086b093c00b`. Studio builds do not depend on a local RageLab checkout.

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
- Asset inspection, capabilities, preview bounds, GTA/FiveM discovery, YMAP workspace scene assembly, provider resolution, scene transforms/collision state, operation planning, and mutation are delegated directly to `ragelab-engine`.
- Scene proxies remain the fallback for unresolved, unsupported, budget-limited, preview-failed, or scale-incomplete nodes. Eligible YDR/YDD nodes use core `AssetPreviewReport` geometry only when the SceneManifest provides an explicit scale.
- Scene preview loading is UI-bounded to 24 unique assets with concurrency 3 and a bounded cache; the asset preview payload itself remains bounded by Rust core policy.
- YBN collision dependencies remain `localOnly` and are not world-placed because the current SceneManifest does not provide an explicit collision placement transform.
- Declarative edit forms are presentation/schema mapping only; a successful core plan is required before apply.
- Studio operation source/output paths must be absolute, and output must differ from source; existing outputs remain rejected by the core create-new policy.
- Core revisions are pinned by public Git commit; committed local path dependencies are not permitted.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

RageLab is an independent open-source project and is not affiliated with or endorsed by Rockstar Games or Take-Two Interactive.
