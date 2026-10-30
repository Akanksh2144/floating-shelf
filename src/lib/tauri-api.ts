import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { Shelf, StageProgressEvent } from "../types/shelf";

export async function getActiveShelf(): Promise<Shelf | null> {
  return invoke("get_active_shelf");
}

export async function createShelf(title?: string): Promise<Shelf> {
  return invoke("create_shelf", { title: title ?? null });
}

export async function stagePaths(shelfId: string, paths: string[]): Promise<Shelf> {
  return invoke("stage_paths", { shelfId, paths });
}

export async function removeItem(shelfId: string, itemId: string): Promise<Shelf> {
  return invoke("remove_item", { shelfId, itemId });
}

export async function startOutboundDrag(
  shelfId: string,
  itemIds: string[],
): Promise<void> {
  return invoke("start_outbound_drag", { shelfId, itemIds });
}

export function onItemProgress(
  handler: (event: StageProgressEvent) => void,
): Promise<UnlistenFn> {
  return listen<StageProgressEvent>("shelf:item-progress", (e) => handler(e.payload));
}
