import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { X, Pin, GripVertical, FileIcon, ImageIcon, FileText } from "lucide-react";
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

function FileKindIcon({ kind }: { kind: string }) {
  if (kind === "image") return <ImageIcon size={16} className="text-blue-600" />;
  if (kind === "document" || kind === "pdf")
    return <FileText size={16} className="text-slate-600" />;
  return <FileIcon size={16} className="text-slate-500" />;
}

function ItemRow({
  item,
  onRemove,
  onDragStart,
}: {
  item: ShelfItem;
  onRemove: () => void;
  onDragStart: () => void;
}) {
  const isBusy = item.status === "copying" || item.status === "transforming";
  const failed = item.status === "failed";

  return (
    <div
      className={`group flex items-center gap-2 rounded-xl px-2 py-2 transition-colors ${
        failed ? "bg-red-50" : "bg-white/80 hover:bg-white"
      } ${item.status === "ready" ? "cursor-grab active:cursor-grabbing" : "cursor-default"}`}
      onPointerDown={(e) => {
        if (item.status !== "ready" || e.button !== 0) return;
        // Delay slightly so click-to-remove still works; outbound drag after move.
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
      <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg bg-slate-100">
        <FileKindIcon kind={item.kind} />
      </div>

      <div className="min-w-0 flex-1">
        <div className="truncate text-[13px] font-medium text-slate-900">{item.name}</div>
        {isBusy ? (
          <div className="mt-1 h-1.5 w-full overflow-hidden rounded-full bg-slate-200">
            <div
              className="h-full rounded-full bg-blue-600 transition-[width] duration-150"
              style={{ width: `${Math.round(item.progress * 100)}%` }}
            />
          </div>
        ) : failed ? (
          <div className="truncate text-[11px] text-red-600">
            {item.error ?? "Copy failed"}
          </div>
        ) : (
          <div className="text-[11px] text-slate-500">
            {item.extension ? item.extension.toUpperCase() : "FILE"} · {formatBytes(item.sizeBytes)}
            {item.status === "ready" ? " · Ready" : ""}
          </div>
        )}
      </div>

      <button
        type="button"
        aria-label="Remove"
        className="rounded-md p-1 text-slate-400 opacity-0 transition group-hover:opacity-100 hover:bg-slate-100 hover:text-slate-700"
        onClick={(e) => {
          e.stopPropagation();
          onRemove();
        }}
        onPointerDown={(e) => e.stopPropagation()}
      >
        <X size={14} />
      </button>
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
          // If brand-new item arrived via event
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
    if (!shelf) return { count: 0, bytes: 0, ready: 0 };
    return {
      count: shelf.items.length,
      bytes: shelf.items.reduce((s, i) => s + i.sizeBytes, 0),
      ready: shelf.items.filter((i) => i.status === "ready").length,
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

  return (
    <div className="flex h-full w-full flex-col p-2">
      <div
        className={`flex h-full flex-col overflow-hidden rounded-2xl border border-[var(--shelf-border)] bg-[var(--shelf-bg)] shadow-[0_12px_40px_rgba(0,0,0,0.18)] backdrop-blur-xl ${
          draggingOver ? "ring-2 ring-blue-500/60" : ""
        }`}
      >
        {/* Header — Fabric/Dropbox style: title + meta + close */}
        <div
          className="flex cursor-grab items-center gap-2 border-b border-black/5 px-3 py-2.5 active:cursor-grabbing"
          onMouseDown={() => void startWindowDrag()}
        >
          <GripVertical size={14} className="text-slate-400" />
          <div className="min-w-0 flex-1">
            <div className="truncate text-[13px] font-semibold text-slate-900">
              {shelf?.title ?? "Shelf"}
            </div>
            <div className="truncate text-[11px] text-slate-500">
              {totals.count === 0
                ? "Drop files to hold"
                : `${totals.count} item${totals.count === 1 ? "" : "s"} · ${formatBytes(totals.bytes)} · ${totals.ready} ready`}
            </div>
          </div>
          <button
            type="button"
            className="rounded-md p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-700"
            title="Pin (coming soon)"
            onMouseDown={(e) => e.stopPropagation()}
          >
            <Pin size={14} />
          </button>
          <button
            type="button"
            className="rounded-md p-1 text-slate-400 hover:bg-slate-100 hover:text-slate-700"
            aria-label="Close"
            onMouseDown={(e) => e.stopPropagation()}
            onClick={() => void closeWindow()}
          >
            <X size={14} />
          </button>
        </div>

        {/* Body — Linktree/Upwork dashed dropzone + Saily file rows */}
        <div
          className="flex min-h-0 flex-1 flex-col gap-2 p-2"
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
          {shelf && shelf.items.length > 0 ? (
            <div className="flex min-h-0 flex-1 flex-col gap-1 overflow-y-auto pr-0.5">
              {shelf.items.map((item) => (
                <ItemRow
                  key={item.id}
                  item={item}
                  onRemove={() => {
                    void (async () => {
                      if (!shelf) return;
                      const updated = await removeItem(shelf.id, item.id);
                      setShelf(updated);
                    })();
                  }}
                  onDragStart={() => {
                    if (!shelf || item.status !== "ready") return;
                    void startOutboundDrag(shelf.id, [item.id]);
                  }}
                />
              ))}
            </div>
          ) : (
            <div
              className={`flex flex-1 flex-col items-center justify-center rounded-xl border-2 border-dashed px-4 text-center transition-colors ${
                draggingOver
                  ? "border-blue-500 bg-blue-50/70"
                  : "border-[var(--shelf-dash)] bg-white/40"
              }`}
            >
              <div className="mb-2 flex h-10 w-10 items-center justify-center rounded-full bg-slate-100">
                <FileIcon size={18} className="text-slate-500" />
              </div>
              <div className="text-[13px] font-medium text-slate-800">
                {draggingOver ? "Release to stage" : "Drop files here"}
              </div>
              <div className="mt-1 text-[11px] leading-snug text-slate-500">
                Copies into the shelf — originals stay untouched
              </div>
            </div>
          )}

          {error && (
            <div className="rounded-lg bg-red-50 px-2 py-1.5 text-[11px] text-red-700">
              {error}
            </div>
          )}
        </div>

        {/* Footer action strip placeholder — Instant Actions later */}
        <div className="border-t border-black/5 px-3 py-2">
          <div className="text-[10px] tracking-wide text-slate-400 uppercase">
            File-safety prototype · Copy in · Copy out
          </div>
        </div>
      </div>
    </div>
  );
}
