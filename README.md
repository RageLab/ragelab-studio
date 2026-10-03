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

The repository currently provides the desktop application shell, native workspace directory selection, GTA V Legacy/FiveM discovery, direct engine-backed asset inspection/capability discovery, bounded YDR/YDD/YBN preview, plan-first declarative Legacy editing, and an engine-backed YMAP workspace scene browser. The scene browser selects a YMAP inside the active workspace, renders core summary/warnings/limits and unresolved reasons, and visualizes core-provided node transforms as interactive Three.js proxies with collision relationship state. Editing always chooses a separate output path, runs a dry-run plan first, invalidates that plan when parameters change, and delegates apply/replan/semantic verification to the Rust core.

The Rust adapter is reproducibly pinned to RageLab core revision `2bea1e6406b41f125061776a4a66bb92f57d8b8f`. Studio builds do not depend on a local RageLab checkout.

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
- Scene proxy geometry is presentation-only: it visualizes returned transforms/resolution state and never substitutes for resolved asset geometry or missing core scale.
- Declarative edit forms are presentation/schema mapping only; a successful core plan is required before apply.
- Studio operation source/output paths must be absolute, and output must differ from source; existing outputs remain rejected by the core create-new policy.
- Core revisions are pinned by public Git commit; committed local path dependencies are not permitted.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

RageLab is an independent open-source project and is not affiliated with or endorsed by Rockstar Games or Take-Two Interactive.
