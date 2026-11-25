import { useEffect, useMemo, useState } from "react";
import * as api from "../lib/tauri";
import type { CategorizeItem } from "../lib/tauri";

/** Items per request: categories appear chunk by chunk on a long list
 *  instead of after one long wait (new texts need the model once). */
const CHUNK = 250;

/** Category per item for a list that isn't in the library (marketplace,
 *  collections, git preview). Fills in progressively; failures leave items
 *  uncategorized, so the list still shows. */
export function useItemCategories(items: CategorizeItem[]): Record<string, string> {
  const [result, setResult] = useState<{ sig: string; map: Record<string, string> }>({ sig: "", map: {} });
  // Re-query only when the set of texts changes, not on every render.
  const sig = useMemo(() => items.map((i) => `${i.key}\u0001${i.text}`).join("\u0002"), [items]);

  useEffect(() => {
    if (!sig) return;
    let cancelled = false;
    const timer = setTimeout(async () => {
      let merged: Record<string, string> = {};
      for (let i = 0; i < items.length && !cancelled; i += CHUNK) {
        try {
          const map = await api.categorizeItems(items.slice(i, i + CHUNK));
          if (cancelled) return;
          merged = { ...merged, ...map };
          setResult({ sig, map: merged });
        } catch {
          return;
        }
      }
    }, 150);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // `items` is captured through `sig`
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [sig]);

  return result.sig === sig ? result.map : {};
}
