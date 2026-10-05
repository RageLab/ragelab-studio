# Native viewport parity

Card 09 promotes the reusable RageLab wgpu surface renderer to the default Studio scene viewport while retaining the previous Three.js implementation as an explicit fallback.

## Integration

The Studio creates a Tauri native `Window` without a WebView, parents it to the main window, removes decorations/taskbar presence and enables cursor pass-through. The wgpu surface is owned by `ragelab-render::SurfaceRenderer`.

The main WebView remains responsible for UI controls and input capture. It forwards normalized orbit/pan/zoom/fly/pick input to Rust and continuously synchronizes the physical viewport rectangle across window movement, resize, scroll and DPI changes.

RAGE parsing and dependency/material semantics stay in RageLab Core. The native viewport receives one shared `RenderPackage`; the Three.js fallback requests bounded `AssetPreviewReport` payloads only when the fallback is selected.

## Real-scene validation

Validation used the unchanged source resource:

`D:\DEV\fivem\SV-dev\resources\[dev]\patins`

and GTA V Legacy through the existing read-only game index/key cache.

### patins_blockout

Native Tauri/wgpu smoke:

- 1 instance;
- 1 GPU asset;
- 1 mesh;
- 2 materials;
- 2 GPU textures;
- 154,172 bytes render payload;
- center picking selected node 0;
- camera orbit/zoom and selection highlight completed successfully.

### hei_sm_23_strm_0

Native Tauri/wgpu smoke:

- 339 instances;
- 48 GPU assets;
- 161 meshes;
- 147 materials;
- 78 GPU textures;
- 18,773,152 bytes render payload;
- all counts match the Core/CLI render-package validation from card 08.

## Benchmark

Measurements below are from a debug build on the validation workstation. They are regression evidence, not universal performance guarantees.

| Metric | Native wgpu | Three.js fallback |
| --- | ---: | ---: |
| Scene nodes | 339 | 339 |
| Unique assets | 48 | 48 |
| End-to-end load | 13,372.6 ms | 169,621.2 ms |
| Relative load | 1.0× | 12.7× slower |
| Renderer payload | 18,773,152 B | 18,117,792 B typed |
| Preview JSON transport | none | 37,970,554 B |
| Sampled frames | 90 | 90 |
| Frame timing | 11.17 ms wall/frame | 8.15 ms JS render-call/frame |

The load comparison is directly relevant to user-visible readiness: native builds one scene package and uploads shared GPU resources once, whereas the fallback assembles the scene and performs per-asset preview requests/JSON transport.

Frame numbers are **not GPU-equivalent measurements**. The native figure includes the Rust command/channel/render/present wall path used by the smoke harness; the Three.js figure measures the JavaScript render-call interval and may not include completed GPU presentation. They are retained as regression indicators, not as a claim that one backend has lower GPU frame time.

## Fallback policy

Native wgpu is the default after the parity gates above. Three.js remains selectable and is activated automatically if native viewport initialization fails. Its preview payloads are not loaded while native mode is active.
