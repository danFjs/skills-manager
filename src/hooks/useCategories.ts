import { useCallback, useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import * as api from "../lib/tauri";
import type { CategoryInfo, CategoryProgress } from "../lib/tauri";

/** Categories and where each library skill landed. Assignments are re-read
 *  whenever the background categorizer reports progress. */
export function useCategories() {
  const [categories, setCategories] = useState<CategoryInfo[]>([]);
  const [assignments, setAssignments] = useState<Record<string, string>>({});
  const [progress, setProgress] = useState<CategoryProgress>({ remaining: 0, unavailable: null });

  const reload = useCallback(async () => {
    const [cats, map] = await Promise.all([api.listCategories(), api.categoryAssignments()]);
    setCategories(cats);
    setAssignments(map);
  }, []);

  useEffect(() => {
    Promise.all([api.listCategories(), api.categoryAssignments()])
      .then(([cats, map]) => {
        setCategories(cats);
        setAssignments(map);
      })
      .catch(() => {});
    const unlisten = listen<CategoryProgress>("categories-updated", (event) => {
      setProgress(event.payload);
      api.categoryAssignments().then(setAssignments).catch(() => {});
    });
    return () => {
      unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);

  const save = useCallback(
    async (input: { id?: string; name: string; emoji: string; description: string }) => {
      const saved = await api.saveCategory(input);
      await reload();
      return saved;
    },
    [reload],
  );

  const remove = useCallback(
    async (id: string) => {
      await api.deleteCategory(id);
      await reload();
    },
    [reload],
  );

  return { categories, assignments, progress, save, remove };
}
