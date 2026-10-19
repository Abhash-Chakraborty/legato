import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import type { TFunction } from "i18next";
import { toast } from "sonner";
import { ArrowUp, Copy, Loader2, RotateCw } from "lucide-react";
import {
  commands,
  type AiStatus,
  type AiStatusKind,
  type Note,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { describeAi } from "../../../lib/utils/aiStatus";
import { useAiStore, watchAi } from "../../../stores/aiStore";
import { useNavStore } from "../../../stores/navStore";
import { Button } from "../../ui/Button";
import { answerParts } from "../../notes/notes";

/** Why a note has no summary, or why Ask can't answer, in words; `fix`
 *  opens Settings › AI. */
const problemText = (
  code: string,
  status: AiStatus | null,
  t: TFunction,
): { text: string; fix: boolean } => {
  switch (code) {
    case "short":
      return { text: t("notes.summary.short"), fix: false };
    case "empty":
      return { text: t("notes.ask.empty"), fix: false };
    case "off":
      return { text: t("notes.summary.off"), fix: true };
    case "none":
      return { text: t("notes.summary.none"), fix: true };
    case "recording":
      return { text: t("notes.summary.recording"), fix: false };
    default: {
      if (!status) return { text: t("notes.summary.failed"), fix: false };
      const line = describeAi({ ...status, kind: code as AiStatusKind }, t);
      return { text: line.text, fix: line.action === "fix" };
    }
  }
};

const QUESTIONS = {
  meeting: ["promised", "blocked", "owners"],
  memo: ["points", "todo"],
} as const;

/** Ask about this note: a question, a few to start from, and the answer
 *  with its times, which open the transcript there. */
const Ask: React.FC<{
  note: Note;
  ready: boolean;
  onTime: (ms: number) => void;
}> = ({ note, ready, onTime }) => {
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
      .askNote(note.id, q)
      .catch((e) => ({ status: "error" as const, error: String(e) }));
    setBusy(false);
    if (result.status === "ok") {
      setAnswer(result.data);
      setQuestion("");
    } else setProblem(result.error);
  };

  const issue = problem ? problemText(problem, status, t) : null;

  return (
    <section aria-labelledby={`ask-${note.id}`} className="mt-8">
      <h3 id={`ask-${note.id}`} className="mb-2.5 text-sm font-medium">
        {t("notes.ask.title")}
      </h3>
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
          disabled={!ready}
          placeholder={t("notes.ask.placeholder")}
          aria-label={t("notes.ask.title")}
          className="input w-full pe-11"
        />
        <button
          type="submit"
          disabled={!ready || busy || !question.trim()}
          aria-label={t("notes.ask.send")}
          title={t("notes.ask.send")}
          className="absolute end-1.5 top-1/2 grid h-7 w-7 -translate-y-1/2 place-items-center rounded-[var(--r-sm)] bg-teal-fill text-white transition-colors duration-[var(--dur-1)] hover:bg-teal-fill-hover disabled:bg-surface-3 disabled:text-text-4"
        >
          <ArrowUp className="h-4 w-4" aria-hidden="true" />
        </button>
      </form>
      {ready && (
        <div className="mt-2.5 flex flex-wrap gap-1.5">
          {QUESTIONS[note.kind].map((id) => (
            <button
              key={id}
              type="button"
              disabled={busy}
              onClick={() => ask(t(`notes.ask.q.${id}`))}
              className="h-7 rounded-full border border-line px-3 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97] disabled:opacity-60"
            >
              {t(`notes.ask.q.${id}`)}
            </button>
          ))}
        </div>
      )}
      {asked && (
        <div className="mt-4 rounded-[var(--r-md)] bg-surface-2 px-4 py-3">
          <p className="text-[13px] font-medium text-text-2">{asked}</p>
          <div aria-live="polite" className="mt-1.5 text-sm leading-relaxed">
            {busy ? (
              <span className="inline-flex items-center gap-2 text-text-3">
                <Loader2
                  className="h-3.5 w-3.5 animate-spin"
                  aria-hidden="true"
                />
                {t("notes.ask.thinking")}
              </span>
            ) : issue ? (
              <span className="text-danger">
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
              </span>
            ) : (
              answer !== null &&
              answerParts(answer).map((part, i) =>
                part.kind === "text" ? (
                  <span key={i} className="whitespace-pre-wrap">
                    {part.text}
                  </span>
                ) : (
                  <button
                    key={i}
                    type="button"
                    onClick={() => onTime(part.ms)}
                    title={t("notes.ask.jump", { time: part.label })}
                    className="mx-0.5 rounded-[var(--r-xs)] bg-teal-50 px-1 text-[13px] font-medium tabular-nums text-teal-700 hover:bg-teal-100"
                  >
                    {part.label}
                  </button>
                ),
              )
            )}
          </div>
        </div>
      )}
    </section>
  );
};

interface SummaryTabProps {
  note: Note;
  onCopy: () => void;
  onTime: (ms: number) => void;
  onShowTranscript: () => void;
}

/** The summary, decisions and action items your AI wrote, Regenerate, and
 *  Ask about this note. */
export const SummaryTab: React.FC<SummaryTabProps> = ({
  note,
  onCopy,
  onTime,
  onShowTranscript,
}) => {
  const { t } = useTranslation();
  const status = useAiStore((s) => s.status);
  const go = useNavStore((s) => s.go);
  const { settings } = useSettings();
  const [regenerating, setRegenerating] = useState(false);
  const [done, setDone] = useState<Record<number, boolean>>({});

  useEffect(watchAi, []);
  useEffect(() => setDone({}), [note.id, note.summary_at]);

  const enabled = settings?.ai_notes_enabled ?? true;
  const hasAi = !!status && status.kind !== "none";
  const summarising = note.status === "summarising" || regenerating;
  const hasSummary = !!note.summary.trim();
  const transcribing =
    note.status === "recording" || note.status === "transcribing";
  // No saved error, but no AI to ask either.
  const code =
    note.summary_error ?? (!enabled ? "off" : !hasAi ? "none" : null);
  const issue = code ? problemText(code, status, t) : null;

  const regenerate = async () => {
    if (regenerating) return;
    setRegenerating(true);
    const result = await commands
      .summariseNote(note.id)
      .catch((e) => ({ status: "error" as const, error: String(e) }));
    setRegenerating(false);
    // Codes such as "unreachable" are saved on the note and shown inline.
    if (result.status === "error" && !/^[a-z_]+$/.test(result.error))
      toast.error(t("notes.toast.summaryFailed"), {
        description: result.error,
      });
  };

  const toggle = async (id: number, next: boolean) => {
    setDone((d) => ({ ...d, [id]: next }));
    const result = await commands.setNoteActionDone(id, next).catch(() => null);
    if (result?.status !== "ok") setDone((d) => ({ ...d, [id]: !next }));
  };

  const actions = note.actions.map((a) => ({
    ...a,
    done: done[a.id] ?? a.done,
  }));
  const doneCount = actions.filter((a) => a.done).length;
  const canAsk = enabled && hasAi && note.lines > 0;

  if (transcribing)
    return (
      <p className="flex items-center gap-2 text-sm text-text-3">
        <Loader2 className="h-4 w-4 animate-spin" aria-hidden="true" />
        {t("notes.summary.transcribing")}
      </p>
    );

  return (
    <div>
      {summarising ? (
        <p
          role="status"
          className="flex items-center gap-2 text-sm text-text-2"
        >
          <Loader2
            className="h-4 w-4 animate-spin text-teal-700"
            aria-hidden="true"
          />
          {t("notes.summary.writing")}
        </p>
      ) : hasSummary ? (
        <>
          <p className="max-w-[640px] text-[15px] leading-relaxed">
            {note.summary}
          </p>
          {note.decisions.length > 0 && (
            <section className="mt-6">
              <h3 className="mb-2 text-sm font-medium">
                {t("notes.summary.decisions")}
              </h3>
              <ul className="grid gap-1.5 ps-1 text-sm">
                {note.decisions.map((d, i) => (
                  <li key={i} className="flex gap-2.5">
                    <span
                      aria-hidden="true"
                      className="mt-[9px] h-1 w-1 flex-none rounded-full bg-text-3"
                    />
                    {d}
                  </li>
                ))}
              </ul>
            </section>
          )}
          {actions.length > 0 && (
            <section className="mt-6">
              <h3 className="mb-1.5 flex items-baseline gap-2 text-sm font-medium">
                {t("notes.summary.actions")}
                <span className="text-xs font-normal tabular-nums text-text-3">
                  {t("notes.summary.doneCount", {
                    done: doneCount,
                    count: actions.length,
                  })}
                </span>
              </h3>
              <ul>
                {actions.map((a) => (
                  <li key={a.id}>
                    <label className="flex min-h-10 items-center gap-3 border-b border-line py-2 text-sm last:border-b-0">
                      <input
                        type="checkbox"
                        className="check"
                        checked={a.done}
                        onChange={(e) => toggle(a.id, e.target.checked)}
                      />
                      <span
                        className={`min-w-0 flex-1 ${a.done ? "text-text-3 line-through" : ""}`}
                      >
                        {a.text}
                      </span>
                      <span
                        className={`flex-none rounded-[var(--r-xs)] px-1.5 py-0.5 text-xs ${
                          a.owner.toLowerCase() === "you"
                            ? "bg-teal-50 text-teal-700"
                            : "bg-surface-2 text-text-2"
                        }`}
                      >
                        {a.owner.toLowerCase() === "you"
                          ? t("notes.who.you")
                          : a.owner}
                      </span>
                    </label>
                  </li>
                ))}
              </ul>
            </section>
          )}
        </>
      ) : (
        <p className="text-sm text-text-2">
          {!issue
            ? t("notes.summary.noneYet")
            : note.summary_error &&
                !["short", "off", "none"].includes(code ?? "")
              ? t("notes.summary.failedWith", { problem: issue.text })
              : issue.text}{" "}
          {note.lines > 0 && !issue?.fix && (
            <button
              type="button"
              onClick={onShowTranscript}
              className="font-medium text-teal-700 underline-offset-2 hover:underline"
            >
              {t("notes.summary.readTranscript")}
            </button>
          )}
        </p>
      )}

      {!summarising && hasSummary && issue && note.summary_error && (
        <p className="mt-4 text-[13px] text-danger">
          {t("notes.summary.lastFailed", { problem: issue.text })}
        </p>
      )}

      {(hasSummary || issue?.fix || note.lines > 0) && (
        <div className="mt-5 flex flex-wrap items-center gap-2 border-t border-line pt-4">
          {hasSummary && (
            <Button variant="secondary" size="sm" onClick={onCopy}>
              <Copy className="h-3.5 w-3.5" aria-hidden="true" />
              {t("notes.summary.copy")}
            </Button>
          )}
          {issue?.fix ? (
            <Button
              variant="secondary"
              size="sm"
              onClick={() => go("settings", "ai")}
            >
              {t("notes.summary.openAi")}
            </Button>
          ) : (
            note.lines > 0 && (
              <Button
                variant="secondary"
                size="sm"
                disabled={summarising}
                onClick={regenerate}
              >
                <RotateCw className="h-3.5 w-3.5" aria-hidden="true" />
                {hasSummary
                  ? t("notes.summary.regenerate")
                  : t("notes.summary.summarise")}
              </Button>
            )
          )}
          {hasSummary && note.summary_model && (
            <span className="ms-1 text-xs text-text-3">
              {t("notes.summary.model", { model: note.summary_model })}
            </span>
          )}
        </div>
      )}

      {note.lines > 0 && <Ask note={note} ready={canAsk} onTime={onTime} />}
    </div>
  );
};
