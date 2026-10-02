import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";

export interface StudioInfo {
  product: string;
  version: string;
}

export function getStudioInfo(): Promise<StudioInfo> {
  return invoke<StudioInfo>("studio_info");
}

export async function chooseWorkspaceDirectory(): Promise<string | null> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Open workspace",
  });

  return typeof selected === "string" ? selected : null;
}
