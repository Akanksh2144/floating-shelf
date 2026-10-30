import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import {
  X,
  GripVertical,
  FileIcon,
  ImageIcon,
  FileText,
  Film,
  Archive,
} from "lucide-react";
import type { Shelf, ShelfItem } from "../types/shelf";
import {
  getActiveShelf,
  createShelf,
  stagePaths,
  removeItem,
  startOutboundDrag,
  onItemProgress,
} from "../lib/tauri-api";

function formatBytes(n: number): string {
  if (n < 1024) return `${n} B`;
  if (n < 1024 * 1024) return `${(n / 1024).toFixed(1)} KB`;
  return `${(n / (1024 * 1024)).toFixed(1)} MB`;
}

function TypeMark({ kind }: { kind: string }) {
  if (kind === "image") return <ImageIcon size={22} className="text-blue-600" />;
  if (kind === "pdf") return <FileText size={22} className="text-red-600" />;
  if (kind === "document") return <FileText size={22} className="text-slate-600" />;
  if (kind === "video") return <Film size={22} className="text-violet-600" />;
  if (kind === "archive") return <Archive size={22} className="text-amber-600" />;
  return <FileIcon size={22} className="text-slate-500" />;
}

function tileWash(kind: string): string {
  if (kind === "pdf") return "bg-red-50";
  if (kind === "video") return "bg-violet-50";
  if (kind === "archive") return "bg-amber-50";
  if (kind === "document") return "bg-slate-100";
  return "bg-slate-100";
}

function ItemTile({
  item,
  onRemove,
  onDragStart,
}: {
  item: ShelfItem;
  onRemove: () => void;
  onDragStart: () => void;
}) {
  const [thumbFailed, setThumbFailed] = useState(false);
  const isBusy = item.status === "copying" || item.status === "transforming";
  const failed = item.status === "failed";
  const showThumb = item.kind === "image" && item.status === "ready" && !thumbFailed;
  const ext = item.extension ? item.extension.toUpperCase() : "FILE";

  return (
    <div
      className={`group relative flex flex-col ${
        item.status === "ready" ? "cursor-grab active:cursor-grabbing" : "cursor-default"
      }`}
      onPointerDown={(e) => {
        if (item.status !== "ready" || e.button !== 0) return;
        const startX = e.clientX;
        const startY = e.clientY;
        const onMove = (ev: PointerEvent) => {
          const dx = Math.abs(ev.clientX - startX);
          const dy = Math.abs(ev.clientY - startY);
          if (dx > 5 || dy > 5) {
            window.removeEventListener("pointermove", onMove);
            window.removeEventListener("pointerup", onUp);
            onDragStart();
          }
        };
        const onUp = () => {
          window.removeEventListener("pointermove", onMove);
          window.removeEventListener("pointerup", onUp);
        };
        window.addEventListener("pointermove", onMove);
        window.addEventListener("pointerup", onUp);
      }}
    >
      <div
        className={`relative aspect-square overflow-hidden rounded-2xl border border-black/5 ${
          failed ? "bg-red-50" : tileWash(item.kind)
        }`}
      >
        {showThumb ? (
          <img
            src={convertFileSrc(item.activePath)}
            alt=""
            className="h-full w-full object-cover"
            draggable={false}
            onError={() => setThumbFailed(true)}
          />
        ) : (
          <div className="flex h-full w-full flex-col items-center justify-center gap-1">
            <TypeMark kind={item.kind} />
            <span className="text-[10px] font-semibold tracking-wide text-slate-500">{ext}</span>
          </div>
        )}

        {isBusy && (
          <div className="absolute inset-x-0 bottom-0 bg-white/90 px-2 py-1.5">
            <div className="h-1 overflow-hidden rounded-full bg-slate-200">
              <div
                className="h-full rounded-full bg-blue-600"
                style={{ width: `${Math.round(item.progress * 100)}%` }}
              />
            </div>
          </div>
        )}

        <button
          type="button"
          aria-label="Remove"
          className="absolute top-1.5 right-1.5 flex h-6 w-6 items-center justify-center rounded-full bg-white/95 text-slate-500 opacity-0 shadow-sm transition group-hover:opacity-100 hover:text-slate-900"
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          <X size={12} />
        </button>
      </div>

      <div className="mt-1.5 truncate px-0.5 text-[12px] font-medium text-slate-900">
        {item.name}
      </div>
      <div className={`truncate px-0.5 text-[11px] ${failed ? "text-red-600" : "text-slate-500"}`}>
        {failed ? (item.error ?? "Copy failed") : `${ext} · ${formatBytes(item.sizeBytes)}`}
      </div>
    </div>
  );
}

export default function ShelfWindow() {
  const [shelf, setShelf] = useState<Shelf | null>(null);
  const [draggingOver, setDraggingOver] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dragDepth = useRef(0);
  const shelfRef = useRef<Shelf | null>(null);

  useEffect(() => {
    shelfRef.current = shelf;
  }, [shelf]);

  const refresh = useCallback(async () => {
    try {
      let active = await getActiveShelf();
      if (!active) {
        active = await createShelf("Shelf");
      }
      setShelf(active);
    } catch (e) {
      setError(String(e));
    }
  }, []);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    let unlistenProgress: (() => void) | undefined;
    let unlistenDrop: (() => void) | undefined;

    void (async () => {
      unlistenProgress = await onItemProgress((ev) => {
        setShelf((prev) => {
          if (!prev || prev.id !== ev.shelfId) return prev;
          const items = prev.items.map((item) => {
            if (item.id !== ev.itemId) return item;
            if (ev.item) return ev.item;
            return {
              ...item,
              progress: ev.progress,
              status: ev.status,
              error: ev.error ?? item.error,
            };
          });
          if (ev.item && !prev.items.some((i) => i.id === ev.itemId)) {
            return { ...prev, items: [...prev.items, ev.item] };
          }
          return { ...prev, items };
        });
      });

      const win = getCurrentWindow();
      unlistenDrop = await win.onDragDropEvent(async (event) => {
        if (event.payload.type === "enter" || event.payload.type === "over") {
          setDraggingOver(true);
        } else if (event.payload.type === "leave") {
          setDraggingOver(false);
        } else if (event.payload.type === "drop") {
          setDraggingOver(false);
          const paths = event.payload.paths;
          if (!paths.length) return;
          let current = shelfRef.current;
          if (!current) {
            current = await createShelf("Shelf");
            setShelf(current);
            shelfRef.current = current;
          }
          try {
            const updated = await stagePaths(current.id, paths);
            setShelf(updated);
          } catch (e) {
            setError(String(e));
            await refresh();
          }
        }
      });
    })();

    return () => {
      unlistenProgress?.();
      unlistenDrop?.();
    };
  }, [refresh]);

  const totals = useMemo(() => {
    if (!shelf) return { count: 0, bytes: 0 };
    return {
      count: shelf.items.length,
      bytes: shelf.items.reduce((s, i) => s + i.sizeBytes, 0),
    };
  }, [shelf]);

  const closeWindow = async () => {
    await getCurrentWindow().hide();
  };

  const startWindowDrag = async () => {
    try {
      await getCurrentWindow().startDragging();
    } catch {
      /* ignore */
    }
  };

  const hasItems = Boolean(shelf && shelf.items.length > 0);

  return (
    <div className="flex h-full w-full">
      <div
        className={`flex h-full w-full flex-col overflow-hidden bg-[var(--shelf-bg)] ${
          draggingOver ? "ring-2 ring-inset ring-blue-500" : ""
        }`}
      >
        <div
          className="flex cursor-grab items-center gap-2 px-3.5 pt-3 pb-2 active:cursor-grabbing"
          onMouseDown={() => void startWindowDrag()}
        >
          <GripVertical size={14} className="text-slate-300" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-[14px] font-semibold tracking-tight text-slate-900">
              {shelf?.title ?? "Shelf"}
            </div>
            <div className="truncate text-[11px] text-slate-500">
              {totals.count === 0
                ? "Drop files in. Originals stay put."
                : `${totals.count} file${totals.count === 1 ? "" : "s"} · ${formatBytes(totals.bytes)} · drag out to copy`}
            </div>
          </div>
          <button
            type="button"
            className="flex h-7 w-7 items-center justify-center rounded-full text-slate-400 hover:bg-slate-100 hover:text-slate-800"
            aria-label="Close"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={() => void closeWindow()}
          >
            <X size={14} />
          </button>
        </div>

        <div
          className="flex min-h-0 flex-1 flex-col px-3 pb-3"
          onDragEnter={(e) => {
            e.preventDefault();
            dragDepth.current += 1;
            setDraggingOver(true);
          }}
          onDragLeave={(e) => {
            e.preventDefault();
            dragDepth.current -= 1;
            if (dragDepth.current <= 0) {
              dragDepth.current = 0;
              setDraggingOver(false);
            }
          }}
          onDragOver={(e) => e.preventDefault()}
        >
          {hasItems && shelf ? (
            <div className="min-h-0 flex-1 overflow-y-auto pr-0.5">
              <div className="grid grid-cols-2 gap-x-2.5 gap-y-3">
                {shelf.items.map((item) => (
                  <ItemTile
                    key={item.id}
                    item={item}
                    onRemove={() => {
                      void (async () => {
                        const updated = await removeItem(shelf.id, item.id);
                        setShelf(updated);
                      })();
                    }}
                    onDragStart={() => {
                      if (item.status !== "ready") return;
                      void startOutboundDrag(shelf.id, [item.id]);
                    }}
                  />
                ))}
              </div>
            </div>
          ) : (
            <div
              className={`flex flex-1 flex-col items-center justify-center rounded-2xl border border-dashed px-6 text-center ${
                draggingOver
                  ? "border-blue-500 bg-blue-50"
                  : "border-[var(--shelf-dash)] bg-[var(--shelf-inset)]"
              }`}
            >
              <div className="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-white shadow-sm">
                <ImageIcon size={20} className="text-blue-600" />
              </div>
              <div className="text-[13px] font-semibold text-slate-900">
                {draggingOver ? "Release to copy in" : "Drop files here"}
              </div>
              <div className="mt-1 max-w-[200px] text-[12px] leading-snug text-slate-500">
                The shelf keeps a copy. Your original file is not moved.
              </div>
            </div>
          )}

          {hasItems && (
            <div
              className={`mt-3 flex shrink-0 items-center justify-center rounded-xl border border-dashed px-3 py-2 text-[12px] font-medium ${
                draggingOver
                  ? "border-blue-500 bg-blue-50 text-blue-700"
                  : "border-[var(--shelf-dash)] bg-[var(--shelf-inset)] text-slate-500"
              }`}
            >
              {draggingOver ? "Release to copy in" : "Drop more files"}
            </div>
          )}

          {error && (
            <div className="mt-2 rounded-lg bg-red-50 px-2 py-1.5 text-[11px] text-red-700">
              {error}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
