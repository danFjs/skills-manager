import { useState } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import type { CategoryInfo } from "../lib/tauri";

/** Mirrors core::semantic::categories::MIN_DESCRIPTION_CHARS. */
const MIN_DESCRIPTION = 20;

interface Draft {
  id?: string;
  name: string;
  emoji: string;
  description: string;
  fallback: boolean;
}

interface Props {
  open: boolean;
  categories: CategoryInfo[];
  onClose: () => void;
  onSave: (draft: { id?: string; name: string; emoji: string; description: string }) => Promise<unknown>;
  onDelete: (id: string) => Promise<void>;
}

/** Add, rename and describe categories. Skills are sorted by meaning against
 *  each description, so one is required; saving re-sorts in the background. */
export function CategoriesDialog({ open, categories, onClose, onSave, onDelete }: Props) {
  const { t } = useTranslation();
  const [editing, setEditing] = useState<Draft | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  if (!open) return null;

  const tooShort =
    editing !== null && !editing.fallback && editing.description.trim().length < MIN_DESCRIPTION;

  const save = async () => {
    if (!editing) return;
    setBusy(true);
    setError(null);
    try {
      await onSave(editing);
      setEditing(null);
    } catch (e) {
      setError(String(e));
    } finally {
      setBusy(false);
    }
  };

  const remove = async (id: string) => {
    setError(null);
    try {
      await onDelete(id);
      if (editing?.id === id) setEditing(null);
    } catch (e) {
      setError(String(e));
    }
  };

  const inputClass =
    "w-full bg-background border border-border-subtle rounded-lg px-3 py-2 text-[13px] text-secondary focus:outline-none focus:border-border transition-all placeholder-faint";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 z-0 bg-black/70 backdrop-blur-sm" onClick={onClose} />
      <div className="relative z-10 flex max-h-[80vh] w-full max-w-[560px] flex-col rounded-xl border border-border bg-surface p-5 shadow-2xl">
        <div className="mb-2 flex items-center justify-between">
          <h2 className="text-[13px] font-semibold text-primary">{t("categories.title")}</h2>
          <button
            onClick={onClose}
            className="rounded-md p-1 text-muted outline-none transition-colors hover:text-secondary"
          >
            <X className="h-4 w-4" />
          </button>
        </div>
        <p className="mb-3 text-[12px] text-muted">{t("categories.hint")}</p>

        <div className="min-h-0 flex-1 space-y-1 overflow-y-auto">
          {categories.map((c) =>
            editing?.id === c.id ? null : (
              <div key={c.id} className="flex items-center gap-3 rounded-lg px-2 py-1.5 hover:bg-surface-hover">
                <span className="w-5 text-center text-[13px]">{c.emoji}</span>
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[13px] font-medium text-primary">{c.name}</div>
                  <div className="truncate text-[12px] text-muted">
                    {c.fallback ? t("categories.otherHint") : c.description}
                  </div>
                </div>
                <button
                  className="rounded-lg px-2 py-1 text-[12px] font-medium text-tertiary transition-colors hover:bg-surface-hover hover:text-secondary"
                  onClick={() =>
                    setEditing({ id: c.id, name: c.name, emoji: c.emoji, description: c.description, fallback: c.fallback })
                  }
                >
                  {t("categories.editOne")}
                </button>
                {!c.fallback && (
                  <button
                    title={t("categories.delete")}
                    onClick={() => remove(c.id)}
                    className="rounded-md p-1 text-muted transition-colors hover:text-red-500"
                  >
                    <X className="h-3.5 w-3.5" />
                  </button>
                )}
              </div>
            )
          )}

          {editing && (
            <div className="space-y-2 rounded-lg border border-border p-3">
              <div className="flex gap-2">
                <input
                  className={`${inputClass} w-14 text-center`}
                  value={editing.emoji}
                  placeholder="🙂"
                  aria-label={t("categories.emoji")}
                  onChange={(e) => setEditing({ ...editing, emoji: e.target.value })}
                />
                <input
                  className={inputClass}
                  value={editing.name}
                  placeholder={t("categories.name")}
                  aria-label={t("categories.name")}
                  autoFocus
                  onChange={(e) => setEditing({ ...editing, name: e.target.value })}
                />
              </div>
              {!editing.fallback && (
                <>
                  <textarea
                    className={`${inputClass} min-h-[84px] resize-y`}
                    value={editing.description}
                    placeholder={t("categories.descriptionPlaceholder")}
                    aria-label={t("categories.description")}
                    onChange={(e) => setEditing({ ...editing, description: e.target.value })}
                  />
                  <p className={`text-[12px] ${tooShort ? "text-red-500" : "text-muted"}`}>
                    {tooShort
                      ? t("categories.descriptionTooShort", { count: MIN_DESCRIPTION })
                      : t("categories.resortHint")}
                  </p>
                </>
              )}
              <div className="flex justify-end gap-2">
                <button
                  onClick={() => setEditing(null)}
                  className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-tertiary outline-none transition-colors hover:bg-surface-hover hover:text-secondary"
                >
                  {t("common.cancel")}
                </button>
                <button
                  onClick={save}
                  disabled={busy || !editing.name.trim() || tooShort}
                  className="rounded-lg border border-accent-border bg-accent-dark px-3 py-1.5 text-[13px] font-medium text-white outline-none transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
                >
                  {editing.id ? t("categories.save") : t("categories.add")}
                </button>
              </div>
            </div>
          )}
          {error && <p className="text-[12px] text-red-500">{error}</p>}
        </div>

        {!editing && (
          <div className="flex justify-end pt-3">
            <button
              onClick={() => setEditing({ name: "", emoji: "", description: "", fallback: false })}
              className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-tertiary outline-none transition-colors hover:bg-surface-hover hover:text-secondary"
            >
              {t("categories.new")}
            </button>
          </div>
        )}
      </div>
    </div>
  );
}
