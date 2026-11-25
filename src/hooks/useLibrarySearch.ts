import { useEffect, useState } from "react";
import * as api from "../lib/tauri";

const DEBOUNCE_MS = 120;

/** Skill id → rank (0 = best) from the backend's ranked search, or null while
 *  the query is empty, the first response hasn't landed, or the backend
 *  failed — callers then keep their own substring matching. `version`
 *  re-runs the query when the library changes. */
export function useLibrarySearch(query: string, version: unknown): Map<string, number> | null {
  const [ranking, setRanking] = useState<Map<string, number> | null>(null);
  const q = query.trim();

  useEffect(() => {
    // Nothing to fetch; the return below already hides any stale ranking.
    if (!q) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      api
        .searchLibrary(q)
        .then((hits) => {
          if (cancelled) return;
          const rank = new Map<string, number>();
          hits.forEach((hit, i) => {
            if (!rank.has(hit.id)) rank.set(hit.id, i);
          });
          setRanking(rank);
        })
        .catch(() => {
          if (!cancelled) setRanking(null);
        });
    }, DEBOUNCE_MS);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [q, version]);

  return q ? ranking : null;
}
