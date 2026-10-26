import React, { useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowUp, Loader2 } from "lucide-react";
import { commands, type Note } from "@/bindings";
import { useAiStore } from "../../../stores/aiStore";
import { useNavStore } from "../../../stores/navStore";
import { Dialog } from "../../ui/Dialog";
import { notesAnswerParts } from "../../notes/notes";
import { problemText } from "./SummaryTab";

const QUESTIONS = ["week", "promised", "open"] as const;

/** Ask across notes: one question about every summarised note, answered
 *  with the notes it comes from, which open on a click. */
export const AskNotes: React.FC<{
  open: boolean;
  notes: Note[];
  onClose: () => void;
  onOpen: (id: number) => void;
}> = ({ open, notes, onClose, onOpen }) => {
  const { t } = useTranslation();
  const status = useAiStore((s) => s.status);
  const go = useNavStore((s) => s.go);
  const [question, setQuestion] = useState("");
  const [asked, setAsked] = useState<string | null>(null);
  const [answer, setAnswer] = useState<string | null>(null);
  const [problem, setProblem] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);
  const ref = useRef<HTMLInputElement>(null);

  const ask = async (text: string) => {
    const q = text.trim();
    if (!q || busy) return;
    setBusy(true);
    setAsked(q);
    setAnswer(null);
    setProblem(null);
    const result = await commands
      .askAllNotes(q)
      .catch((e) => ({ status: "error" as const, error: String(e) }));
    setBusy(false);
    if (result.status === "ok") {
      setAnswer(result.data);
      setQuestion("");
    } else setProblem(result.error);
  };

  const issue =
    problem === "nosummaries"
      ? { text: t("notes.askAll.noSummaries"), fix: false }
      : problem
        ? problemText(problem, status, t)
        : null;
  const title = (id: number) =>
    notes.find((n) => n.id === id)?.title ?? t("notes.askAll.note", { id });

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={t("notes.askAll.title")}
      description={t("notes.askAll.description")}
      closeLabel={t("common.close")}
      initialFocusRef={ref}
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          ask(question);
        }}
        className="relative"
      >
        <input
          ref={ref}
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          placeholder={t("notes.askAll.placeholder")}
          aria-label={t("notes.askAll.title")}
          className="input w-full pe-11"
        />
        <button
          type="submit"
          disabled={busy || !question.trim()}
          aria-label={t("notes.ask.send")}
          title={t("notes.ask.send")}
          className="absolute end-1.5 top-1/2 grid h-7 w-7 -translate-y-1/2 place-items-center rounded-[var(--r-sm)] bg-teal-fill text-white transition-colors duration-[var(--dur-1)] hover:bg-teal-fill-hover disabled:bg-surface-3 disabled:text-text-4"
        >
          <ArrowUp className="h-4 w-4" aria-hidden="true" />
        </button>
      </form>
      <div className="mt-2.5 flex flex-wrap gap-1.5">
        {QUESTIONS.map((id) => (
          <button
            key={id}
            type="button"
            disabled={busy}
            onClick={() => ask(t(`notes.askAll.q.${id}`))}
            className="h-7 rounded-full border border-line px-3 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97] disabled:opacity-60"
          >
            {t(`notes.askAll.q.${id}`)}
          </button>
        ))}
      </div>
      {asked && (
        <div className="mt-4 rounded-[var(--r-md)] bg-surface-2 px-4 py-3">
          <p className="text-[13px] font-medium text-text-2">{asked}</p>
          <div
            aria-live="polite"
            className="mt-1.5 select-text text-sm leading-relaxed"
          >
            {busy ? (
              <span className="inline-flex items-center gap-2 text-text-3">
                <Loader2
                  className="h-3.5 w-3.5 animate-spin"
                  aria-hidden="true"
                />
                {t("notes.askAll.reading")}
              </span>
            ) : issue ? (
              <span className="text-danger">
                {issue.text}
                {issue.fix && (
                  <>
                    {" "}
                    <button
                      type="button"
                      onClick={() => {
                        onClose();
                        go("settings", "ai");
                      }}
                      className="font-medium text-teal-700 underline-offset-2 hover:underline"
                    >
                      {t("notes.summary.openAi")}
                    </button>
                  </>
                )}
              </span>
            ) : (
              answer !== null &&
              notesAnswerParts(answer).map((part, i) =>
                part.kind === "text" ? (
                  <span key={i} className="whitespace-pre-wrap">
                    {part.text}
                  </span>
                ) : (
                  <button
                    key={i}
                    type="button"
                    onClick={() => onOpen(part.id)}
                    title={t("notes.tasks.open")}
                    className="mx-0.5 inline-block max-w-[220px] truncate rounded-[var(--r-xs)] bg-teal-50 px-1 align-bottom text-[13px] font-medium text-teal-700 hover:bg-teal-100"
                  >
                    {title(part.id)}
                  </button>
                ),
              )
            )}
          </div>
        </div>
      )}
    </Dialog>
  );
};
