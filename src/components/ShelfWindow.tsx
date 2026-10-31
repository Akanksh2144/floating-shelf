import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { convertFileSrc } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AnimatePresence, motion } from "motion/react";
import {
  X,
  FileIcon,
  ImageIcon,
  FileText,
  Film,
  Archive,
  ChevronUp,
} from "lucide-react";
import type { Shelf, ShelfItem } from "../types/shelf";
import {
  getActiveShelf,
  createShelf,
  stagePaths,
  removeItem,
  startOutboundDrag,
  onItemProgress,
  onIslandMode,
} from "../lib/tauri-api";
import { ISLAND, morphIslandWindow, snapIslandWindow } from "../lib/island";

const IDLE_MS = 10_000;

const easeOut = { type: "spring" as const, stiffness: 420, damping: 34, mass: 0.8 };

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
  index,
}: {
  item: ShelfItem;
  onRemove: () => void;
  onDragStart: () => void;
  index: number;
}) {
  const [thumbFailed, setThumbFailed] = useState(false);
  const isBusy = item.status === "copying" || item.status === "transforming";
  const failed = item.status === "failed";
  const showThumb = item.kind === "image" && item.status === "ready" && !thumbFailed;
  const ext = item.extension ? item.extension.toUpperCase() : "FILE";

  return (
    <motion.div
      layout
      initial={{ opacity: 0, y: 12, scale: 0.96 }}
      animate={{ opacity: 1, y: 0, scale: 1 }}
      exit={{ opacity: 0, scale: 0.94, transition: { duration: 0.15 } }}
      transition={{ ...easeOut, delay: Math.min(index * 0.04, 0.28) }}
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
              <motion.div
                className="h-full rounded-full bg-blue-600"
                initial={false}
                animate={{ width: `${Math.round(item.progress * 100)}%` }}
                transition={{ type: "spring", stiffness: 220, damping: 28 }}
              />
            </div>
          </div>
        )}

        <motion.button
          type="button"
          aria-label="Remove"
          whileHover={{ scale: 1.06 }}
          whileTap={{ scale: 0.92 }}
          className="absolute top-1.5 right-1.5 flex h-6 w-6 items-center justify-center rounded-full bg-white/95 text-slate-500 opacity-0 shadow-sm transition-opacity group-hover:opacity-100 hover:text-slate-900"
          onClick={(e) => {
            e.stopPropagation();
            onRemove();
          }}
          onPointerDown={(e) => e.stopPropagation()}
        >
          <X size={12} />
        </motion.button>
      </div>

      <div className="mt-1.5 truncate px-0.5 text-[12px] font-medium leading-4 text-slate-900">
        {item.name}
      </div>
      <div
        className={`truncate px-0.5 text-[11px] leading-4 ${failed ? "text-red-600" : "text-slate-500"}`}
      >
        {failed ? (item.error ?? "Copy failed") : `${ext} · ${formatBytes(item.sizeBytes)}`}
      </div>
    </motion.div>
  );
}

export default function ShelfWindow() {
  const [shelf, setShelf] = useState<Shelf | null>(null);
  const [mode, setMode] = useState<"collapsed" | "expanded">("collapsed");
  const [draggingOver, setDraggingOver] = useState(false);
  const [hovered, setHovered] = useState(false);
  const [outboundDragging, setOutboundDragging] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const dragDepth = useRef(0);
  const shelfRef = useRef<Shelf | null>(null);
  const modeRef = useRef(mode);
  const morphingRef = useRef(false);
  const sizeRef = useRef<{ w: number; h: number }>({ ...ISLAND.collapsed });

  useEffect(() => {
    shelfRef.current = shelf;
  }, [shelf]);

  useEffect(() => {
    modeRef.current = mode;
  }, [mode]);

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

  const expand = useCallback(async (opts?: { instant?: boolean }) => {
    if (modeRef.current === "expanded" || morphingRef.current) return;
    morphingRef.current = true;
    try {
      setMode("expanded");
      if (opts?.instant) {
        await snapIslandWindow(true);
      } else {
        await morphIslandWindow(true, sizeRef.current);
      }
      sizeRef.current = ISLAND.expanded;
    } catch (e) {
      setError(String(e));
      setMode("collapsed");
    } finally {
      morphingRef.current = false;
    }
  }, []);

  const collapse = useCallback(async () => {
    if (modeRef.current === "collapsed" || morphingRef.current) return;
    // Never collapse while a file drag is over the shelf.
    if (draggingOver) return;
    morphingRef.current = true;
    try {
      setDraggingOver(false);
      dragDepth.current = 0;
      setMode("collapsed");
      await new Promise((r) => window.setTimeout(r, 90));
      await morphIslandWindow(false, sizeRef.current);
      sizeRef.current = ISLAND.collapsed;
    } catch (e) {
      setError(String(e));
      setMode("expanded");
    } finally {
      morphingRef.current = false;
    }
  }, [draggingOver]);

  useEffect(() => {
    void refresh();
    void morphIslandWindow(false).then(() => {
      sizeRef.current = ISLAND.collapsed;
      setMode("collapsed");
    });
  }, [refresh]);

  useEffect(() => {
    let unlisten: (() => void) | undefined;
    void onIslandMode((next) => {
      if (morphingRef.current) return;
      setMode(next);
      sizeRef.current = next === "expanded" ? ISLAND.expanded : ISLAND.collapsed;
    }).then((fn) => {
      unlisten = fn;
    });
    return () => unlisten?.();
  }, []);

  const isBusy = useMemo(
    () =>
      Boolean(
        shelf?.items.some(
          (i) => i.status === "copying" || i.status === "transforming",
        ),
      ),
    [shelf],
  );

  useEffect(() => {
    if (mode !== "expanded") return;
    if (hovered || draggingOver || isBusy || outboundDragging || morphingRef.current) return;

    const id = window.setTimeout(() => {
      void collapse();
    }, IDLE_MS);
    return () => window.clearTimeout(id);
  }, [mode, hovered, draggingOver, isBusy, outboundDragging, collapse]);

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
        // Never resize/move the HWND during enter/over/drop. Doing so mid
        // DoDragDrop wedges Explorer's drag loop system-wide until we exit.
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
          // Expand only after OLE drop has finished.
          window.setTimeout(() => {
            void expand({ instant: true });
          }, 150);
        }
      });
    })();

    return () => {
      unlistenProgress?.();
      unlistenDrop?.();
    };
  }, [refresh, expand]);

  const totals = useMemo(() => {
    if (!shelf) return { count: 0, bytes: 0 };
    return {
      count: shelf.items.length,
      bytes: shelf.items.reduce((s, i) => s + i.sizeBytes, 0),
    };
  }, [shelf]);

  const startWindowDrag = async () => {
    try {
      await getCurrentWindow().startDragging();
    } catch {
      /* ignore */
    }
  };

  const hasItems = Boolean(shelf && shelf.items.length > 0);
  const expanded = mode === "expanded";

  // Collapsed: thin vertical edge bar only (no labels — they won't fit).
  if (!expanded) {
    return (
      <button
        type="button"
        aria-label="Expand Modillion"
        className={`h-full w-full cursor-pointer rounded-full border border-black/10 outline-none ${
          draggingOver ? "bg-blue-100" : "bg-white"
        }`}
        onClick={() => void expand()}
        onMouseEnter={() => setHovered(true)}
        onMouseLeave={() => setHovered(false)}
      />
    );
  }

  return (
    <div
      className="flex h-full w-full flex-col overflow-hidden rounded-[22px] border border-black/10 bg-white"
      onMouseEnter={() => setHovered(true)}
      onMouseLeave={() => setHovered(false)}
    >
      <div
        className="flex min-h-[52px] shrink-0 cursor-grab items-center gap-2.5 px-3.5 py-2 active:cursor-grabbing"
        onMouseDown={() => void startWindowDrag()}
      >
        <span className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full bg-slate-900 text-[11px] font-bold tracking-tight text-white">
          M
        </span>

        <div className="min-w-0 flex-1 overflow-visible">
          <div className="flex flex-col justify-center gap-0.5">
            <div className="truncate text-[14px] font-semibold leading-5 tracking-tight text-slate-900">
              Modillion
            </div>
            <div className="truncate text-[11px] leading-4 text-slate-500">
              {totals.count === 0
                ? "Drop files in. Originals stay put."
                : `${totals.count} file${totals.count === 1 ? "" : "s"} · ${formatBytes(totals.bytes)} · drag out to copy`}
            </div>
          </div>
        </div>

        <motion.button
          type="button"
          aria-label="Collapse"
          whileHover={{ scale: 1.06, backgroundColor: "rgb(241 245 249)" }}
          whileTap={{ scale: 0.92 }}
          className="flex h-7 w-7 items-center justify-center rounded-full text-slate-400 hover:text-slate-800"
          onMouseDown={(e) => e.stopPropagation()}
          onClick={(e) => {
            e.stopPropagation();
            void collapse();
          }}
        >
          <ChevronUp size={14} />
        </motion.button>
      </div>

      <motion.div
        key="body"
        initial={{ opacity: 0, x: 16 }}
        animate={{ opacity: 1, x: 0 }}
        transition={easeOut}
        className={`flex min-h-0 flex-1 flex-col px-3 pb-3 ${
          draggingOver ? "ring-2 ring-inset ring-blue-500" : ""
        }`}
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
            <motion.div layout className="grid grid-cols-2 gap-x-2.5 gap-y-3">
              <AnimatePresence mode="popLayout">
                {shelf.items.map((item, index) => (
                  <ItemTile
                    key={item.id}
                    item={item}
                    index={index}
                    onRemove={() => {
                      void (async () => {
                        const updated = await removeItem(shelf.id, item.id);
                        setShelf(updated);
                      })();
                    }}
                    onDragStart={() => {
                      if (item.status !== "ready") return;
                      setOutboundDragging(true);
                      void startOutboundDrag(shelf.id, [item.id]).finally(() => {
                        setOutboundDragging(false);
                      });
                    }}
                  />
                ))}
              </AnimatePresence>
            </motion.div>
          </div>
        ) : (
          <motion.div
            layout
            initial={{ opacity: 0, scale: 0.98 }}
            animate={{ opacity: 1, scale: 1 }}
            transition={easeOut}
            className={`flex flex-1 flex-col items-center justify-center rounded-2xl border border-dashed px-6 text-center ${
              draggingOver
                ? "border-blue-500 bg-blue-50"
                : "border-[var(--shelf-dash)] bg-[var(--shelf-inset)]"
            }`}
          >
            <motion.div
              className="mb-3 flex h-12 w-12 items-center justify-center rounded-2xl bg-white shadow-sm"
              animate={draggingOver ? { scale: 1.06, y: -2 } : { scale: 1, y: 0 }}
              transition={easeOut}
            >
              <ImageIcon size={20} className="text-blue-600" />
            </motion.div>
            <div className="text-[13px] font-semibold text-slate-900">
              {draggingOver ? "Release to copy in" : "Drop files here"}
            </div>
            <div className="mt-1 max-w-[200px] text-[12px] leading-snug text-slate-500">
              The shelf keeps a copy. Your original file is not moved.
            </div>
          </motion.div>
        )}

        {hasItems && (
          <motion.div
            layout
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            className={`mt-3 flex shrink-0 items-center justify-center rounded-xl border border-dashed px-3 py-2 text-[12px] font-medium ${
              draggingOver
                ? "border-blue-500 bg-blue-50 text-blue-700"
                : "border-[var(--shelf-dash)] bg-[var(--shelf-inset)] text-slate-500"
            }`}
          >
            {draggingOver ? "Release to copy in" : "Drop more files"}
          </motion.div>
        )}

        {error && (
          <motion.div
            initial={{ opacity: 0, y: 6 }}
            animate={{ opacity: 1, y: 0 }}
            className="mt-2 rounded-lg bg-red-50 px-2 py-1.5 text-[11px] text-red-700"
          >
            {error}
          </motion.div>
        )}
      </motion.div>
    </div>
  );
}
