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
- GTA V Legacy and FiveM environment discovery.

### RageLab Studio

This repository owns:

- desktop lifecycle through Tauri;
- local workspace selection and desktop permissions;
- Svelte application state and interaction;
- Three.js rendering;
- editor presentation, comparison, and recovery UX;
- native command adapters required by desktop workflows.

## Invariant

A RAGE operation is not complete if it can only be executed through RageLab Studio.

Reusable behavior must be implemented in RageLab first and then exposed to the desktop application. The Studio may explain a core capability or rejection reason, but it must not recreate binary-layout or writer-safety decisions.

## Native boundary

The frontend communicates with Rust through typed Tauri commands.

```text
Svelte / Three.js
       |
       | invoke
       v
Tauri commands
       |
       v
RageLab Rust APIs
```

Filesystem access, GTA/FiveM discovery, asset inspection, capability discovery, bounded preview construction, workspace indexing, mutation planning, and write operations belong on the native side of this boundary. Tauri commands should be thin wrappers that serialize core reports; they must not reinterpret format metadata, writer eligibility, preview hard caps, or discovery evidence.

## Repository independence

RageLab Studio must build from its own repository. It must not depend on local paths to development clones or private workspaces. The committed core dependency uses the public RageLab Git repository pinned to an exact commit revision; `StudioInfo.coreRevision` exposes that revision to the desktop shell. Development against unreleased RageLab code may use a documented local override, but that override must never be committed.
