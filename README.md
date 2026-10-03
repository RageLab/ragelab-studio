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

The repository currently provides the desktop application shell, native workspace directory selection, GTA V Legacy/FiveM discovery, and direct engine-backed asset inspection, capability discovery, and bounded YDR/YDD/YBN preview commands through a typed frontend/native bridge.

The Rust adapter is reproducibly pinned to RageLab core revision `15db923f11ab87737984c93f97e486b766bd4959`. Studio builds do not depend on a local RageLab checkout.

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
- Asset inspection, capabilities, preview bounds, and GTA/FiveM discovery are delegated directly to `ragelab-engine`.
- Core revisions are pinned by public Git commit; committed local path dependencies are not permitted.

## License

Apache License 2.0. See [LICENSE](LICENSE) and [NOTICE](NOTICE).

RageLab is an independent open-source project and is not affiliated with or endorsed by Rockstar Games or Take-Two Interactive.
