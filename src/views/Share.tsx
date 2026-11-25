import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { open as dialogOpen, save as dialogSave } from "@tauri-apps/plugin-dialog";
import { FileDown, FileUp, Loader2 } from "lucide-react";
import { toast } from "sonner";
import { useApp } from "../context/AppContext";
import * as api from "../lib/tauri";
import { getErrorMessage } from "../lib/error";
import { cn } from "../utils";

/** Local sharing (replaces git backup in this fork): export library skills
 *  to one zip, or import such a zip. Only skill files travel — no account,
 *  no network, no tags or presets. */
export function Share() {
  const { t } = useTranslation();
  const { managedSkills, refreshManagedSkills } = useApp();
  const [query, setQuery] = useState("");
  // Deselected ids; everything starts selected so "export all" is one click.
  const [excluded, setExcluded] = useState<Set<string>>(new Set());
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [progress, setProgress] = useState<{ current: number; total: number } | null>(null);

  const sorted = useMemo(
    () => [...managedSkills].sort((a, b) => a.name.localeCompare(b.name)),
    [managedSkills],
  );
  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return q ? sorted.filter((s) => s.name.toLowerCase().includes(q)) : sorted;
  }, [sorted, query]);
  const selected = sorted.filter((s) => !excluded.has(s.id));

  const toggle = (id: string) =>
    setExcluded((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });

  const handleExport = async () => {
    if (selected.length === 0) return;
    const dest = await dialogSave({
      defaultPath: "skills.zip",
      filters: [{ name: "Zip", extensions: ["zip"] }],
    });
    if (!dest) return;
    setExporting(true);
    try {
      const ids = selected.length === sorted.length ? [] : selected.map((s) => s.id);
      const summary = await api.exportSkillsArchive(ids, dest);
      toast.success(t("share.exported", { count: summary.skills }));
    } catch (error) {
      toast.error(getErrorMessage(error, t("common.error")));
    } finally {
      setExporting(false);
    }
  };

  const handleImport = async () => {
    const picked = await dialogOpen({
      multiple: false,
      filters: [{ name: "Zip", extensions: ["zip", "skill"] }],
    });
    if (!picked || Array.isArray(picked)) return;
    setImporting(true);
    const unlisten = await listen<{ current: number; total: number }>(
      "batch-import-progress",
      (event) => setProgress(event.payload),
    );
    try {
      const result = await api.importSkillsArchive(picked);
      await refreshManagedSkills();
      if (result.imported > 0) {
        toast.success(t("share.imported", { count: result.imported }));
      }
      if (result.skipped > 0) {
        toast.info(t("share.skipped", { count: result.skipped }));
      }
      if (result.errors.length > 0) {
        console.warn("Import issues:", result.errors);
        toast.error(t("share.importFailed", { count: result.errors.length }));
      }
    } catch (error) {
      toast.error(getErrorMessage(error, t("common.error")));
    } finally {
      unlisten();
      setProgress(null);
      setImporting(false);
    }
  };

  return (
    <div className="app-page gap-4">
      <div className="app-page-header border-b-0 pb-0">
        <h1 className="app-page-title mb-1">{t("share.title")}</h1>
        <p className="text-[13px] text-muted">{t("share.subtitle")}</p>
      </div>

      <section className="app-panel max-w-2xl p-5">
        <div className="mb-3 flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h2 className="text-[14px] font-semibold text-primary">{t("share.exportTitle")}</h2>
            <p className="mt-0.5 text-[13px] text-muted">{t("share.exportDesc")}</p>
          </div>
          <button
            type="button"
            onClick={handleExport}
            disabled={exporting || selected.length === 0}
            className="app-button-primary shrink-0"
          >
            {exporting ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <FileDown className="h-3.5 w-3.5" />}
            {t("share.exportButton", { count: selected.length })}
          </button>
        </div>

        {sorted.length === 0 ? (
          <p className="text-[13px] text-muted">{t("share.empty")}</p>
        ) : (
          <>
            <div className="mb-2 flex items-center gap-2">
              <input
                type="text"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
                placeholder={t("share.filter")}
                className="app-input w-full bg-background"
              />
              <button
                type="button"
                className="app-button-secondary shrink-0"
                onClick={() =>
                  setExcluded(excluded.size === 0 ? new Set(sorted.map((s) => s.id)) : new Set())
                }
              >
                {excluded.size === 0 ? t("share.selectNone") : t("share.selectAll")}
              </button>
            </div>
            <div className="max-h-[360px] space-y-0.5 overflow-y-auto">
              {shown.map((skill) => {
                const on = !excluded.has(skill.id);
                return (
                  <label
                    key={skill.id}
                    className={cn(
                      "flex cursor-pointer items-center gap-3 rounded-lg px-3 py-1.5 transition-colors hover:bg-surface-hover",
                      !on && "opacity-60",
                    )}
                  >
                    <input type="checkbox" checked={on} onChange={() => toggle(skill.id)} />
                    <span className="min-w-0 flex-1">
                      <span className="block truncate text-[13px] font-medium text-primary">{skill.name}</span>
                      {skill.description && (
                        <span className="block truncate text-[12px] text-muted">{skill.description}</span>
                      )}
                    </span>
                  </label>
                );
              })}
            </div>
          </>
        )}
      </section>

      <section className="app-panel max-w-2xl p-5">
        <div className="flex items-start justify-between gap-3">
          <div className="min-w-0">
            <h2 className="text-[14px] font-semibold text-primary">{t("share.importTitle")}</h2>
            <p className="mt-0.5 text-[13px] text-muted">{t("share.importDesc")}</p>
          </div>
          <button
            type="button"
            onClick={handleImport}
            disabled={importing}
            className="app-button-secondary shrink-0"
          >
            {importing ? <Loader2 className="h-3.5 w-3.5 animate-spin" /> : <FileUp className="h-3.5 w-3.5" />}
            {importing && progress
              ? t("share.importProgress", { current: progress.current, total: progress.total })
              : t("share.importButton")}
          </button>
        </div>
      </section>
    </div>
  );
}
