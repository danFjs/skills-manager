import { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, ExternalLink, Loader2, Plus, Search } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { CategoryPills } from "./CategoryPills";
import { Pager } from "./Pager";
import { useItemCategories } from "../hooks/useItemCategories";
import {
  catalogInstallUrl,
  catalogKey,
  loadSkillCatalog,
  matchesCatalogQuery,
  mergeUserCollections,
  type CatalogSkill,
} from "../lib/skillCatalog";
import type { CategoryInfo, CategoryProgress, UserCollection } from "../lib/tauri";

const PAGE_SIZE = 24;

interface Props {
  categories: CategoryInfo[];
  progress: CategoryProgress;
  onEditCategories: () => void;
  /** Lowercased names of skills already in the library. */
  installedNames: Set<string>;
  /** Your own collections; their skills are listed first. */
  userCollections: UserCollection[];
  /** Opens the git install preview for one skill. */
  onInstall: (url: string) => void;
  busy: boolean;
}

/** Every skill from the curated collection repos as its own card — with
 *  description, category and one-click install. The repos themselves are
 *  listed (uncategorized) on the Git tab. */
export function CollectionSkills({
  categories,
  progress,
  onEditCategories,
  installedNames,
  userCollections,
  onInstall,
  busy,
}: Props) {
  const { t } = useTranslation();
  const [shipped, setShipped] = useState<CatalogSkill[] | null>(null);
  const [query, setQuery] = useState("");
  const [category, setCategory] = useState<string | null>(null);
  const [page, setPage] = useState(1);

  useEffect(() => {
    loadSkillCatalog().then(setShipped).catch(() => setShipped([]));
  }, []);

  const catalog = useMemo(
    () => (shipped === null ? null : mergeUserCollections(shipped, userCollections)),
    [shipped, userCollections],
  );
  const matched = useMemo(
    () => (catalog ?? []).filter((s) => matchesCatalogQuery(s, query)),
    [catalog, query],
  );
  const items = useMemo(
    () =>
      matched.map((s) => ({
        key: catalogKey(s),
        text: `${s.name.replace(/[-_]/g, " ")}. ${s.description}`.trim(),
      })),
    [matched],
  );
  const assigned = useItemCategories(items);
  const counts = useMemo(() => {
    const m = new Map<string, number>();
    for (const s of matched) {
      const c = assigned[catalogKey(s)];
      if (c) m.set(c, (m.get(c) ?? 0) + 1);
    }
    return m;
  }, [matched, assigned]);
  const shown = category ? matched.filter((s) => assigned[catalogKey(s)] === category) : matched;
  const pages = Math.max(1, Math.ceil(shown.length / PAGE_SIZE));
  const current = Math.min(page, pages);
  const visible = shown.slice((current - 1) * PAGE_SIZE, current * PAGE_SIZE);
  const categorized = [...counts.values()].reduce((a, b) => a + b, 0);

  if (catalog === null) {
    return (
      <div className="flex items-center justify-center py-16">
        <Loader2 className="h-5 w-5 animate-spin text-muted" />
      </div>
    );
  }

  return (
    <div className="animate-in fade-in space-y-3 pb-8 duration-300">
      <div className="app-panel space-y-3 p-3">
        <div className="relative">
          <Search className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-muted" />
          <input
            type="text"
            value={query}
            onChange={(e) => {
              setQuery(e.target.value);
              setPage(1);
            }}
            placeholder={t("install.collectionSkills.search", { count: catalog.length })}
            className="app-input w-full bg-background pl-8"
          />
        </div>
        <p className="text-[12px] text-muted">
          {t("install.collectionSkills.summary", { count: shown.length })}
          {categorized < matched.length && ` · ${t("install.collectionSkills.sorting", { done: categorized, total: matched.length })}`}
        </p>
        {categories.length > 0 && counts.size > 0 && (
          <CategoryPills
            categories={categories}
            counts={counts}
            selected={category}
            onSelect={(c) => {
              setCategory(c);
              setPage(1);
            }}
            onEdit={onEditCategories}
            progress={progress}
          />
        )}
      </div>

      {visible.length === 0 ? (
        <p className="py-10 text-center text-[13px] text-muted">{t("install.collectionSkills.none")}</p>
      ) : (
        <div className="grid grid-cols-1 gap-3 lg:grid-cols-2">
          {visible.map((s) => {
            const installed = installedNames.has(s.name.toLowerCase());
            return (
              <div key={catalogKey(s)} className="app-panel flex min-h-[148px] flex-col gap-2.5 p-4 transition-colors hover:border-border">
                <div className="flex items-start justify-between gap-2">
                  <h3 className="min-w-0 truncate text-[13px] font-semibold text-secondary">{s.name}</h3>
                  <div className="flex shrink-0 items-center gap-1">
                    <button
                      onClick={() => openUrl(catalogInstallUrl(s)).catch(() => {})}
                      className="rounded-md p-1 text-muted transition-colors hover:bg-surface-hover hover:text-secondary"
                      title={t("install.viewOnWeb")}
                    >
                      <ExternalLink className="h-3.5 w-3.5" />
                    </button>
                    {installed ? (
                      <span
                        className="rounded-md border border-emerald-500/20 bg-emerald-500/10 p-1 text-emerald-600 dark:text-emerald-400"
                        title={t("install.installed")}
                      >
                        <Check className="h-3.5 w-3.5" />
                      </span>
                    ) : (
                      <button
                        onClick={() => onInstall(catalogInstallUrl(s))}
                        disabled={busy}
                        className="rounded-md border border-accent-border bg-accent-dark p-1 text-white transition-colors hover:bg-accent disabled:opacity-50"
                        title={t("install.oneClickInstall")}
                      >
                        <Plus className="h-3.5 w-3.5" />
                      </button>
                    )}
                  </div>
                </div>
                {s.description && (
                  <p className="line-clamp-3 text-[13px] leading-5 text-muted">{s.description}</p>
                )}
                <div className="mt-auto flex flex-wrap items-center gap-1">
                  <span className="rounded-full bg-accent-bg px-1.5 py-0.5 text-[12px] font-medium leading-4 text-accent-light">
                    @{s.repo}
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      )}

      <Pager
        current={current}
        total={pages}
        onChange={(p) => {
          setPage(p);
          window.scrollTo({ top: 0, behavior: "smooth" });
        }}
      />
    </div>
  );
}
