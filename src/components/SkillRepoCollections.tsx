import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { FolderGit2 } from "lucide-react";
import repos from "../data/skill-repos.json";

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
}

/** A curated list of GitHub repos that publish skills. Picking one runs the
 *  normal git install preview, so installing works exactly like a pasted URL
 *  (one clone, no GitHub API quota). */
export function SkillRepoCollections({ onBrowse, disabled }: Props) {
  const { t } = useTranslation();
  const [query, setQuery] = useState("");
  const list = repos as SkillRepo[];
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q
      ? list.filter((r) => r.repo.toLowerCase().includes(q) || r.title.toLowerCase().includes(q))
      : list;
  }, [list, query]);

  return (
    <div className="app-panel mt-4 max-w-lg p-5">
      <h2 className="mb-1 text-[14px] font-semibold text-primary">{t("install.collectionsTitle")}</h2>
      <p className="mb-3 text-[13px] text-muted">
        {t("install.collectionsDesc", { count: list.length })}
      </p>
      <input
        type="text"
        value={query}
        onChange={(e) => setQuery(e.target.value)}
        placeholder={t("install.collectionsSearch")}
        className="app-input mb-3 w-full bg-background"
      />
      <div className="max-h-[420px] space-y-1 overflow-y-auto">
        {shown.map((r) => (
          <button
            key={r.repo}
            disabled={disabled}
            onClick={() => onBrowse(`https://github.com/${r.repo}`)}
            className="flex w-full items-center gap-3 rounded-lg border border-transparent px-3 py-2 text-left transition-colors hover:border-border hover:bg-surface-hover disabled:opacity-50"
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
        {shown.length === 0 && (
          <p className="px-3 py-2 text-[13px] text-muted">{t("install.collectionsNone")}</p>
        )}
      </div>
    </div>
  );
}
