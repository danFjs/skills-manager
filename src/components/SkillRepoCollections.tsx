import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, FolderGit2, Loader2, Pencil, Plus, RefreshCw, X } from "lucide-react";
import { toast } from "sonner";
import repos from "../data/skill-repos.json";
import { getErrorMessage } from "../lib/error";
import type { useUserCollections } from "../hooks/useUserCollections";

interface SkillRepo {
  repo: string;
  title: string;
  /** Skills last counted in the repo; null when never indexed. */
  skills: number | null;
}

interface Props {
  /** Opens the repo in the regular git preview (pick skills, then install). */
  onBrowse: (repoUrl: string) => void;
  disabled: boolean;
  mine: ReturnType<typeof useUserCollections>;
}

/** GitHub repos that publish skills: your own collections (add, rename,
 *  refresh, remove) above the ones shipped with the app. Picking one runs
 *  the normal git install preview. Repos are folders — not categorized. */
export function SkillRepoCollections({ onBrowse, disabled, mine }: Props) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const [adding, setAdding] = useState("");
  const [renaming, setRenaming] = useState<{ repo: string; title: string } | null>(null);
  const shipped = repos as SkillRepo[];
  const q = query.trim().toLowerCase();
  const mineRepos = useMemo(() => new Set(mine.collections.map((c) => c.repo.toLowerCase())), [mine.collections]);
  const shownMine = mine.collections.filter(
    (c) => !q || c.repo.toLowerCase().includes(q) || c.title.toLowerCase().includes(q),
  );
  const shownShipped = shipped.filter(
    (r) => !mineRepos.has(r.repo.toLowerCase()) && (!q || r.repo.toLowerCase().includes(q) || r.title.toLowerCase().includes(q)),
  );

  const add = async () => {
    const repo = adding.trim();
    if (!repo) return;
    try {
      const c = await mine.add(repo);
      setAdding("");
      toast.success(t("install.myCollections.added", { name: c.title, count: c.skills.length }));
    } catch (e) {
      toast.error(getErrorMessage(e, t("common.error")));
    }
  };

  const refresh = async (repo: string) => {
    try {
      const c = await mine.refresh(repo);
      toast.success(t("install.myCollections.refreshed", { name: c.title, count: c.skills.length }));
    } catch (e) {
      toast.error(getErrorMessage(e, t("common.error")));
    }
  };

  const saveRename = async () => {
    if (!renaming) return;
    try {
      await mine.rename(renaming.repo, renaming.title);
      setRenaming(null);
    } catch (e) {
      toast.error(getErrorMessage(e, t("common.error")));
    }
  };

  const remove = async (repo: string) => {
    try {
      await mine.remove(repo);
    } catch (e) {
      toast.error(getErrorMessage(e, t("common.error")));
    }
  };

  const rowClass =
    "flex w-full items-center gap-3 rounded-lg border border-transparent px-3 py-2 text-left transition-colors hover:border-border hover:bg-surface-hover";
  const iconButton = "rounded-md p-1 text-muted transition-colors hover:bg-surface-active hover:text-secondary disabled:opacity-50";

  return (
    <div className="app-panel mt-4 max-w-lg p-5">
      <h2 className="mb-1 text-[14px] font-semibold text-primary">{t("install.collectionsTitle")}</h2>
      <p className="mb-3 text-[13px] text-muted">
        {t("install.collectionsDesc", { count: shipped.length + mine.collections.length })}
      </p>

      <div className="mb-3 flex gap-2">
        <input
          type="text"
          value={adding}
          onChange={(e) => setAdding(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && add()}
          placeholder={t("install.myCollections.addPlaceholder")}
          spellCheck={false}
          className="app-input w-full bg-background"
          disabled={mine.busy !== null}
        />
        <button
          type="button"
          onClick={add}
          disabled={!adding.trim() || mine.busy !== null}
          className="app-button-primary shrink-0"
        >
          {mine.busy === adding.trim() && adding.trim() ? (
            <Loader2 className="h-3.5 w-3.5 animate-spin" />
          ) : (
            <Plus className="h-3.5 w-3.5" />
          )}
          {t("install.myCollections.add")}
        </button>
      </div>

      <input
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder={t("install.collectionsSearch")}
        className="app-input mb-3 w-full bg-background"
      />

      <div className="max-h-[420px] space-y-1 overflow-y-auto">
        {shownMine.length > 0 && (
          <p className="px-3 pt-1 text-[12px] font-medium text-tertiary">{t("install.myCollections.title")}</p>
        )}
        {shownMine.map((c) => (
          <div key={c.repo} className={rowClass}>
            <FolderGit2 className="h-4 w-4 shrink-0 text-accent" />
            {renaming?.repo === c.repo ? (
              <input
                className="app-input min-w-0 flex-1 bg-background"
                value={renaming.title}
                autoFocus
                onChange={(e) => setRenaming({ repo: c.repo, title: e.target.value })}
                onKeyDown={(e) => {
                  if (e.key === "Enter") saveRename();
                  if (e.key === "Escape") setRenaming(null);
                }}
              />
            ) : (
              <button
                type="button"
                disabled={disabled}
                onClick={() => onBrowse(`https://github.com/${c.repo}`)}
                className="min-w-0 flex-1 text-left disabled:opacity-50"
              >
                <span className="block truncate text-[13px] font-medium text-primary">{c.title}</span>
                <span className="block truncate text-[12px] text-muted">{c.repo}</span>
              </button>
            )}
            <span className="shrink-0 text-[12px] text-muted">
              {t("install.collectionsSkills", { count: c.skills.length })}
            </span>
            {renaming?.repo === c.repo ? (
              <button type="button" className={iconButton} onClick={saveRename} title={t("install.myCollections.save")}>
                <Check className="h-3.5 w-3.5" />
              </button>
            ) : (
              <button
                type="button"
                className={iconButton}
                onClick={() => setRenaming({ repo: c.repo, title: c.title })}
                title={t("install.myCollections.rename")}
              >
                <Pencil className="h-3.5 w-3.5" />
              </button>
            )}
            <button
              type="button"
              className={iconButton}
              disabled={mine.busy !== null}
              onClick={() => refresh(c.repo)}
              title={t("install.myCollections.refresh")}
            >
              <RefreshCw className={`h-3.5 w-3.5 ${mine.busy === c.repo ? "animate-spin" : ""}`} />
            </button>
            <button
              type="button"
              className={iconButton}
              disabled={mine.busy !== null}
              onClick={() => remove(c.repo)}
              title={t("install.myCollections.remove")}
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        ))}

        {shownMine.length > 0 && shownShipped.length > 0 && (
          <p className="px-3 pt-3 text-[12px] font-medium text-tertiary">{t("install.myCollections.shipped")}</p>
        )}
        {shownShipped.map((r) => (
          <button
            key={r.repo}
            disabled={disabled}
            onClick={() => onBrowse(`https://github.com/${r.repo}`)}
            className={`${rowClass} disabled:opacity-50`}
          >
            <FolderGit2 className="h-4 w-4 shrink-0 text-tertiary" />
            <span className="min-w-0 flex-1">
              <span className="block truncate text-[13px] font-medium text-primary">{r.title}</span>
              <span className="block truncate text-[12px] text-muted">{r.repo}</span>
            </span>
            {r.skills !== null && (
              <span className="shrink-0 text-[12px] text-muted">
                {t("install.collectionsSkills", { count: r.skills })}
              </span>
            )}
          </button>
        ))}
        {shownMine.length === 0 && shownShipped.length === 0 && (
          <p className="px-3 py-2 text-[13px] text-muted">{t("install.collectionsNone")}</p>
        )}
      </div>
    </div>
  );
}
