import { useEffect, useState } from "react";
import { X, Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import * as api from "../lib/tauri";
import { getErrorMessage } from "../lib/error";
import { useOptionalApp } from "../context/AppContext";

interface Props {
  /** The skill to edit; null closes the dialog. */
  skillId: string | null;
  skillName?: string;
  /** Skills from git or skills.sh are overwritten by their next update. */
  sourceType?: string;
  onClose: () => void;
}

/** Edit a library skill's SKILL.md in place. Linked agents see the change
 *  immediately; agents holding copies are re-copied on save. */
export function SkillEditorDialog({ skillId, skillName, sourceType, onClose }: Props) {
  const { t } = useTranslation();
  // Optional: the detail panel also renders outside the app provider.
  const app = useOptionalApp();
  const [content, setContent] = useState<string | null>(null);
  const [original, setOriginal] = useState("");
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (!skillId) return;
    let cancelled = false;
    api
      .getSkillDocument(skillId)
      .then((doc) => {
        if (cancelled) return;
        setContent(doc.content);
        setOriginal(doc.content);
      })
      .catch((error: unknown) => {
        if (!cancelled) toast.error(getErrorMessage(error, t("common.error")));
      });
    return () => {
      cancelled = true;
    };
  }, [skillId, t]);

  if (!skillId) return null;

  const close = () => {
    setContent(null);
    onClose();
  };

  const save = async () => {
    if (content === null) return;
    setSaving(true);
    try {
      await api.saveSkillDocument(skillId, content);
      await app?.refreshManagedSkills();
      toast.success(t("authoring.saved"));
      close();
    } catch (error: unknown) {
      toast.error(getErrorMessage(error, t("common.error")));
    } finally {
      setSaving(false);
    }
  };

  const overwrittenOnUpdate = sourceType === "git" || sourceType === "skillssh";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 z-0 bg-black/70 backdrop-blur-sm" onClick={close} />
      <div className="relative z-10 flex h-[80vh] w-full max-w-[820px] flex-col rounded-xl border border-border bg-surface p-5 shadow-2xl">
        <div className="mb-3 flex items-center justify-between gap-3">
          <h2 className="truncate text-[13px] font-semibold text-primary">
            {t("authoring.editTitle", { name: skillName ?? "" })}
          </h2>
          <button onClick={close} className="rounded-md p-1 text-muted outline-none transition-colors hover:text-secondary">
            <X className="h-4 w-4" />
          </button>
        </div>
        {overwrittenOnUpdate && (
          <p className="mb-2 text-[12px] text-amber-600 dark:text-amber-400">{t("authoring.updateOverwrites")}</p>
        )}
        {content === null ? (
          <div className="flex flex-1 items-center justify-center text-muted">
            <Loader2 className="h-4 w-4 animate-spin" />
          </div>
        ) : (
          <textarea
            value={content}
            onChange={(e) => setContent(e.target.value)}
            spellCheck={false}
            className="min-h-0 flex-1 resize-none rounded-lg border border-border-subtle bg-background p-3 font-mono text-[12px] leading-5 text-secondary outline-none focus:border-border"
          />
        )}
        <div className="flex justify-end gap-2 pt-3">
          <button
            onClick={close}
            className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-tertiary outline-none transition-colors hover:bg-surface-hover hover:text-secondary"
          >
            {t("common.cancel")}
          </button>
          <button
            onClick={save}
            disabled={saving || content === null || content === original}
            className="rounded-lg border border-accent-border bg-accent-dark px-3 py-1.5 text-[13px] font-medium text-white outline-none transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
          >
            {saving ? t("authoring.saving") : t("authoring.save")}
          </button>
        </div>
      </div>
    </div>
  );
}
