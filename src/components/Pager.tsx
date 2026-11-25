import { useTranslation } from "react-i18next";
import { ChevronLeft, ChevronRight } from "lucide-react";
import { cn } from "../utils";
import { visiblePages } from "../lib/pagination";

interface Props {
  current: number;
  total: number;
  onChange: (page: number) => void;
}

/** The Leaderboard's page flipper, shared by every paged skill list. */
export function Pager({ current, total, onChange }: Props) {
  const { t } = useTranslation();
  if (total <= 1) return null;
  const pages = visiblePages(total, current);
  return (
    <div className="mt-5 flex flex-wrap items-center justify-center gap-1.5">
      <button
        onClick={() => onChange(Math.max(1, current - 1))}
        disabled={current === 1}
        className="inline-flex items-center gap-1 rounded-lg border border-border-subtle bg-surface px-3 py-1.5 text-[13px] font-medium text-secondary transition-colors hover:bg-surface-hover disabled:opacity-50"
      >
        <ChevronLeft className="h-3.5 w-3.5" />
        {t("install.pagination.previous")}
      </button>
      {pages.map((page, index) => {
        const showGap = index > 0 && page - pages[index - 1] > 1;
        return (
          <div key={page} className="flex items-center gap-1.5">
            {showGap ? <span className="px-1 text-[13px] text-faint">...</span> : null}
            <button
              onClick={() => onChange(page)}
              className={cn(
                "min-w-8 rounded-lg border px-2.5 py-1.5 text-[13px] font-semibold transition-colors",
                page === current
                  ? "border-accent-border bg-accent-dark text-white"
                  : "border-border-subtle bg-surface text-secondary hover:bg-surface-hover"
              )}
            >
              {page}
            </button>
          </div>
        );
      })}
      <button
        onClick={() => onChange(Math.min(total, current + 1))}
        disabled={current === total}
        className="inline-flex items-center gap-1 rounded-lg border border-border-subtle bg-surface px-3 py-1.5 text-[13px] font-medium text-secondary transition-colors hover:bg-surface-hover disabled:opacity-50"
      >
        {t("install.pagination.next")}
        <ChevronRight className="h-3.5 w-3.5" />
      </button>
    </div>
  );
}
