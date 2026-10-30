export type ItemStatus = "copying" | "ready" | "failed" | "transforming";

export interface ShelfItem {
  id: string;
  name: string;
  extension: string;
  kind: string;
  sizeBytes: number;
  status: ItemStatus;
  progress: number;
  error?: string | null;
  activePath: string;
  stagedInputPath: string;
  originalSourcePath?: string | null;
  createdAt: string;
}

export interface Shelf {
  id: string;
  title: string;
  items: ShelfItem[];
  isPinned: boolean;
  createdAt: string;
  updatedAt: string;
}

export interface StageProgressEvent {
  shelfId: string;
  itemId: string;
  progress: number;
  status: ItemStatus;
  error?: string | null;
  item?: ShelfItem | null;
}
