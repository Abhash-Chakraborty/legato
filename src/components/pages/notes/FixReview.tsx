import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Loader2 } from "lucide-react";
import { commands, type Note, type SuggestedFix } from "@/bindings";
import { useAiStore } from "../../../stores/aiStore";
import { useNavStore } from "../../../stores/navStore";
import { Button } from "../../ui/Button";
import { clock, fixContext } from "../../notes/notes";
import { problemText } from "./SummaryTab";

/** Your AI reads the transcript for misheard words; each fix it finds is
 *  shown in its line, ticked, and nothing changes until Fix. */
export const FixReview: React.FC<{
  note: Note;
  onJump: (lineId: number) => void;
  onClose: () => void;
}> = ({ note, onJump, onClose }) => {
  const { t } = useTranslation();
  const status = useAiStore((s) => s.status);
  const go = useNavStore((s) => s.go);
  const [fixes, setFixes] = useState<SuggestedFix[] | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [skipped, setSkipped] = useState<Set<number>>(new Set());
  const [applying, setApplying] = useState(false);

  useEffect(() => {
    let live = true;
    setFixes(null);
    setProblem(null);
    setSkipped(new Set());
    commands
      .suggestNoteFixes(note.id)
      .catch((e) => ({ status: "error" as const, error: String(e) }))
      .then((r) => {
        if (!live) return;
        if (r.status === "ok") setFixes(r.data);
        else setProblem(r.error);
      });
    return () => {
      live = false;
    };
  }, [note.id]);

  const chosen = (fixes ?? []).filter((_, i) => !skipped.has(i));

  const apply = async () => {
    if (!chosen.length || applying) return;
    setApplying(true);
    const result = await commands.applyNoteFixes(note.id, chosen);
    setApplying(false);
    if (result.status === "error") {
      toast.error(t("notes.aiFix.failed"), { description: result.error });
      return;
    }
    const { undo_token } = result.data;
    toast(t("notes.aiFix.fixed", { count: chosen.length }), {
      action: {
        label: t("notes.toast.undo"),
        onClick: () => commands.undoNoteWordFix(undo_token),
      },
    });
    onClose();
  };

  const issue = problem ? problemText(problem, status, t) : null;

  return (
    <section
      aria-label={t("notes.aiFix.title")}
      className="mb-4 rounded-[var(--r-md)] border border-line bg-surface px-4 py-3"
    >
      {fixes === null && !issue ? (
        <p
          role="status"
          className="flex items-center gap-2 text-sm text-text-2"
        >
          <Loader2
            className="h-4 w-4 animate-spin text-teal-700 motion-reduce:animate-none"
            aria-hidden="true"
          />
          {t("notes.aiFix.reading")}
        </p>
      ) : issue ? (
        <p className="text-sm text-danger">
          {issue.text}
          {issue.fix && (
            <>
              {" "}
              <button
                type="button"
                onClick={() => go("settings", "ai")}
                className="font-medium text-teal-700 underline-offset-2 hover:underline"
              >
                {t("notes.summary.openAi")}
              </button>
            </>
          )}
        </p>
      ) : fixes && fixes.length === 0 ? (
        <p className="text-sm text-text-2">{t("notes.aiFix.none")}</p>
      ) : (
        <>
          <p className="mb-2 text-sm font-medium">
            {t("notes.aiFix.found", { count: fixes?.length ?? 0 })}
          </p>
          <ul className="max-h-[320px] overflow-y-auto">
            {fixes?.map((fix, i) => {
              const context = fixContext(fix.text, fix.heard);
              const on = !skipped.has(i);
              return (
                <li
                  key={`${fix.line_id}-${i}`}
                  className="flex items-baseline gap-3 border-b border-line py-2 text-sm last:border-b-0"
                >
                  <input
                    type="checkbox"
                    className="check translate-y-[3px]"
                    checked={on}
                    aria-label={t("notes.aiFix.pick", {
                      heard: fix.heard,
                      word: fix.word,
                    })}
                    onChange={() =>
                      setSkipped((s) => {
                        const next = new Set(s);
                        if (on) next.add(i);
                        else next.delete(i);
                        return next;
                      })
                    }
                  />
                  <button
                    type="button"
                    onClick={() => onJump(fix.line_id)}
                    title={t("notes.aiFix.show")}
                    className="flex-none rounded-[var(--r-xs)] px-1 text-xs tabular-nums text-text-3 hover:bg-surface-3 hover:text-text"
                  >
                    {clock(fix.at_ms)}
                  </button>
                  <span className={`min-w-0 flex-1 ${on ? "" : "opacity-60"}`}>
                    {context ? (
                      <>
                        <span className="text-text-2">{context.before}</span>
                        <s className="text-text-3">{context.match}</s>{" "}
                        <span className="font-medium text-teal-700">
                          {fix.word}
                        </span>
                        <span className="text-text-2">{context.after}</span>
                      </>
                    ) : (
                      <>
                        <s className="text-text-3">{fix.heard}</s>{" "}
                        <span className="font-medium text-teal-700">
                          {fix.word}
                        </span>
                      </>
                    )}
                  </span>
                </li>
              );
            })}
          </ul>
        </>
      )}
      <div className="mt-3 flex flex-wrap items-center gap-2">
        {fixes && fixes.length > 0 && (
          <Button
            variant="primary"
            size="sm"
            disabled={!chosen.length || applying}
            onClick={apply}
          >
            {t("notes.aiFix.apply", { count: chosen.length })}
          </Button>
        )}
        <Button variant="ghost" size="sm" onClick={onClose}>
          {fixes && fixes.length > 0
            ? t("notes.aiFix.cancel")
            : t("notes.aiFix.close")}
        </Button>
        {fixes && fixes.length > 0 && (
          <span className="ms-1 text-xs text-text-3">
            {t("notes.aiFix.hint")}
          </span>
        )}
      </div>
    </section>
  );
};
