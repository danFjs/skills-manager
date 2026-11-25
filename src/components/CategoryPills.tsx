import { useTranslation } from "react-i18next";
import { Pencil } from "lucide-react";
import { cn } from "../utils";
import type { CategoryInfo, CategoryProgress } from "../lib/tauri";

interface Props {
  categories: CategoryInfo[];
  /** Category id → skills in it, for the list as currently filtered. */
  counts: Map<string, number>;
  selected: string | null;
  onSelect: (id: string | null) => void;
  onEdit: () => void;
  progress: CategoryProgress;
  className?: string;
}

/** Semantic category filter: one pill per non-empty category with its
 *  count, styled like the Channel pills. */
export function CategoryPills({ categories, counts, selected, onSelect, onEdit, progress, className }: Props) {
  const { t } = useTranslation();
  const shown = categories.filter((c) => (counts.get(c.id) ?? 0) > 0 || c.id === selected);

  return (
    <div className={cn("flex flex-wrap items-center gap-1.5", className)}>
      {shown.map((c) => {
        const active = selected === c.id;
        return (
          <button
            key={c.id}
            type="button"
            aria-pressed={active}
            title={c.description}
            onClick={() => onSelect(active ? null : c.id)}
            className={cn(
              "rounded-full px-2.5 py-0.5 text-[12px] font-medium transition-colors",
              active
                ? "bg-accent text-white dark:bg-accent dark:text-white"
                : "bg-surface-hover text-muted hover:text-secondary"
            )}
          >
            {c.emoji ? `${c.emoji} ` : ""}
            {c.name}
            <span className={cn("ml-1", active ? "text-white/80" : "text-faint")}>{counts.get(c.id) ?? 0}</span>
          </button>
        );
      })}
      {progress.remaining > 0 && (
        <span className="animate-pulse text-[12px] text-muted" title={t("categories.sortingHint")}>
          {t("categories.sorting", { count: progress.remaining })}
        </span>
      )}
      {progress.unavailable && (
        <span className="text-[12px] text-muted" title={progress.unavailable}>
          {t("categories.unavailable")}
        </span>
      )}
      <button
        type="button"
        onClick={onEdit}
        title={t("categories.edit")}
        className="rounded-full p-1 text-muted transition-colors hover:bg-surface-hover hover:text-secondary"
      >
        <Pencil className="h-3 w-3" />
      </button>
    </div>
  );
}
