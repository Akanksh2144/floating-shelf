import { animate } from "motion/react";
import { setIslandBounds } from "./tauri-api";

export const ISLAND = {
  collapsed: { w: 28, h: 120 },
  expanded: { w: 340, h: 460 },
} as const;

const spring = {
  type: "spring" as const,
  stiffness: 380,
  damping: 36,
  mass: 0.85,
};

/** Morph the native window between collapsed edge bar and expanded shelf. */
export async function morphIslandWindow(
  expanded: boolean,
  from?: { w: number; h: number },
): Promise<void> {
  const start = from ?? (expanded ? ISLAND.collapsed : ISLAND.expanded);
  const end = expanded ? ISLAND.expanded : ISLAND.collapsed;

  let lastW = start.w;
  let lastH = start.h;

  await animate(0, 1, {
    ...spring,
    onUpdate: (t) => {
      const w = start.w + (end.w - start.w) * t;
      const h = start.h + (end.h - start.h) * t;
      if (Math.abs(w - lastW) < 0.4 && Math.abs(h - lastH) < 0.4) return;
      lastW = w;
      lastH = h;
      // Skip restyle mid-morph — SetWindowRgn every frame kills OLE drop targets.
      void setIslandBounds(w, h, false);
    },
  });

  await setIslandBounds(end.w, end.h, true);
}

/** Instant resize for drag-enter — keeps the HWND stable under the cursor. */
export async function snapIslandWindow(expanded: boolean): Promise<void> {
  const end = expanded ? ISLAND.expanded : ISLAND.collapsed;
  await setIslandBounds(end.w, end.h, true);
}
