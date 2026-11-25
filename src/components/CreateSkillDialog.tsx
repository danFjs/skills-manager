import { useMemo, useState } from "react";
import { X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import * as api from "../lib/tauri";
import type { ManagedSkill } from "../lib/tauri";
import { getErrorMessage } from "../lib/error";
import { useApp } from "../context/AppContext";
import { cn } from "../utils";

interface Props {
  open: boolean;
  onClose: () => void;
  /** The new skill, so the caller can open it in the editor. */
  onCreated: (skill: ManagedSkill) => void;
}

/** Create a skill in the library from a SKILL.md template and, optionally,
 *  link it to agents right away. */
export function CreateSkillDialog({ open, onClose, onCreated }: Props) {
  const { t } = useTranslation();
  const { tools, refreshManagedSkills } = useApp();
  const [name, setName] = useState("");
  const [description, setDescription] = useState("");
  const [agents, setAgents] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const available = useMemo(() => tools.filter((tool) => tool.installed && tool.enabled), [tools]);

  if (!open) return null;

  const reset = () => {
    setName("");
    setDescription("");
    setAgents(new Set());
    setError(null);
  };
  const close = () => {
    reset();
    onClose();
  };

  const toggleAgent = (key: string) =>
    setAgents((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });

  const create = async () => {
    setBusy(true);
    setError(null);
    try {
      const skill = await api.createSkill(name.trim(), description.trim());
      const failed: string[] = [];
      for (const key of agents) {
        await api.syncSkillToTool(skill.id, key).catch(() => failed.push(key));
      }
      await refreshManagedSkills();
      if (failed.length > 0) toast.error(t("authoring.linkFailed", { count: failed.length }));
      toast.success(t("authoring.created", { name: skill.name }));
      reset();
      onCreated(skill);
    } catch (e: unknown) {
      setError(getErrorMessage(e, t("common.error")));
    } finally {
      setBusy(false);
    }
  };

  const inputClass =
    "w-full bg-background border border-border-subtle rounded-lg px-3 py-2 text-[13px] text-secondary focus:outline-none focus:border-border transition-all placeholder-faint";

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center">
      <div className="absolute inset-0 z-0 bg-black/70 backdrop-blur-sm" onClick={close} />
      <div className="relative z-10 w-full max-w-[480px] rounded-xl border border-border bg-surface p-5 shadow-2xl">
        <div className="mb-4 flex items-center justify-between">
          <h2 className="text-[13px] font-semibold text-primary">{t("authoring.createTitle")}</h2>
          <button onClick={close} className="rounded-md p-1 text-muted outline-none transition-colors hover:text-secondary">
            <X className="h-4 w-4" />
          </button>
        </div>
        <div className="space-y-3">
          <div>
            <label className="mb-1 block text-[13px] font-medium text-tertiary">{t("authoring.name")}</label>
            <input
              className={inputClass}
              value={name}
              placeholder="my-skill"
              spellCheck={false}
              autoFocus
              onChange={(e) => setName(e.target.value)}
            />
            <p className="mt-1 text-[12px] text-muted">{t("authoring.nameHint")}</p>
          </div>
          <div>
            <label className="mb-1 block text-[13px] font-medium text-tertiary">{t("authoring.description")}</label>
            <textarea
              className={`${inputClass} min-h-[72px] resize-y`}
              value={description}
              placeholder={t("authoring.descriptionPlaceholder")}
              onChange={(e) => setDescription(e.target.value)}
            />
          </div>
          {available.length > 0 && (
            <div>
              <label className="mb-1 block text-[13px] font-medium text-tertiary">{t("authoring.linkTo")}</label>
              <div className="flex flex-wrap gap-1.5">
                {available.map((tool) => {
                  const on = agents.has(tool.key);
                  return (
                    <button
                      key={tool.key}
                      type="button"
                      aria-pressed={on}
                      onClick={() => toggleAgent(tool.key)}
                      className={cn(
                        "rounded-full px-2.5 py-0.5 text-[12px] font-medium transition-colors",
                        on ? "bg-accent text-white" : "bg-surface-hover text-muted hover:text-secondary"
                      )}
                    >
                      {tool.display_name}
                    </button>
                  );
                })}
              </div>
            </div>
          )}
          {error && <p className="text-[12px] text-red-500">{error}</p>}
          <div className="flex justify-end gap-2 pt-1">
            <button
              onClick={close}
              className="rounded-lg px-3 py-1.5 text-[13px] font-medium text-tertiary outline-none transition-colors hover:bg-surface-hover hover:text-secondary"
            >
              {t("common.cancel")}
            </button>
            <button
              onClick={create}
              disabled={busy || !name.trim() || !description.trim()}
              className="rounded-lg border border-accent-border bg-accent-dark px-3 py-1.5 text-[13px] font-medium text-white outline-none transition-colors hover:bg-accent disabled:cursor-not-allowed disabled:opacity-50"
            >
              {busy ? t("authoring.creating") : t("authoring.createAndEdit")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
