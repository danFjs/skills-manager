import { useCallback, useEffect, useState } from "react";
import * as api from "../lib/tauri";
import type { UserCollection } from "../lib/tauri";

/** Collections you added yourself, and the actions that change them. Each
 *  action re-reads the stored list so every view stays in step. */
export function useUserCollections() {
  const [collections, setCollections] = useState<UserCollection[]>([]);
  /** Repo currently being fetched (add/refresh), for a spinner. */
  const [busy, setBusy] = useState<string | null>(null);

  const reload = useCallback(async () => {
    setCollections(await api.listUserCollections());
  }, []);

  useEffect(() => {
    api.listUserCollections().then(setCollections).catch(() => {});
  }, []);

  const run = useCallback(
    async <T,>(key: string, action: () => Promise<T>): Promise<T> => {
      setBusy(key);
      try {
        const result = await action();
        await reload();
        return result;
      } finally {
        setBusy(null);
      }
    },
    [reload],
  );

  return {
    collections,
    busy,
    reload,
    add: (repo: string) => run(repo, () => api.addUserCollection(repo)),
    refresh: (repo: string) => run(repo, () => api.refreshUserCollection(repo)),
    rename: (repo: string, title: string) => run(repo, () => api.renameUserCollection(repo, title)),
    remove: (repo: string) => run(repo, () => api.removeUserCollection(repo)),
  };
}
