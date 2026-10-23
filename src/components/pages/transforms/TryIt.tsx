import React, { useEffect, useId, useState } from "react";
import { useTranslation } from "react-i18next";
import { History, Play } from "lucide-react";
import {
  commands,
  type AiStatus,
  type PostProcessProvider,
  type Transform,
} from "@/bindings";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { DiffText } from "../../shared/DiffText";
import { finalText } from "../home/format";
import { formatRunTime } from "./list";
import { modelLine, providerFor } from "./model";

type Outcome =
  | { kind: "text"; before: string; text: string; ms: number }
  | { kind: "problem"; text: string };

/** The words for a provider problem, as the bar says them. */
export const useProblemText = () => {
  const { t } = useTranslation();
  return (problem: string, provider: { id: string; label: string } | null) =>
    t(
      provider?.id === "ollama" && problem === "unreachable"
        ? "overlay.notice.problem.unreachableOllama"
        : `overlay.notice.problem.${problem}`,
      {
        name:
          provider?.id === "custom"
            ? t("ai.yourServer")
            : (provider?.label ?? t("ai.provider")),
      },
    );
};

interface TryItProps {
  transforms: Transform[];
  providers: PostProcessProvider[];
  status: AiStatus | null;
  /** The transform picked when it opens. */
  initialId?: string;
  /** Inside the editor: no picker or frame, one column. */
  compact?: boolean;
}

/** Run any transform on text you type or a recent dictation, and see the
 *  changes word by word. Nothing is pasted or recorded. */
export const TryIt: React.FC<TryItProps> = ({
  transforms,
  providers,
  status,
  initialId,
  compact = false,
}) => {
  const { t } = useTranslation();
  const problemText = useProblemText();
  const textId = useId();
  const [id, setId] = useState(initialId ?? transforms[0]?.id ?? "");
  const [text, setText] = useState("");
  const [running, setRunning] = useState(false);
  const [outcome, setOutcome] = useState<Outcome | null>(null);
  const [view, setView] = useState<"changes" | "result">("changes");
  const [noRecent, setNoRecent] = useState(false);

  const transform = transforms.find((x) => x.id === id) ?? transforms[0];
  useEffect(() => setOutcome(null), [id]);

  const loadRecent = async () => {
    const result = await commands
      .getHistoryEntries({
        cursor: null,
        limit: 5,
        search: null,
        app: null,
      })
      .catch(() => null);
    const entry =
      result?.status === "ok"
        ? result.data.entries.find((e) => finalText(e).trim())
        : undefined;
    if (entry) {
      setText(finalText(entry));
      setOutcome(null);
    }
    setNoRecent(!entry);
  };

  const run = async () => {
    if (!transform || !text.trim() || running) return;
    setRunning(true);
    const before = text;
    const result = await commands
      .tryTransform(transform, before)
      .catch(() => null);
    setRunning(false);
    if (!result || result.status === "error") {
      setOutcome({
        kind: "problem",
        text: result?.error ?? t("transforms.try.failed"),
      });
      return;
    }
    const { text: after, ms, problem } = result.data;
    if (after !== null) setOutcome({ kind: "text", before, text: after, ms });
    else
      setOutcome({
        kind: "problem",
        text: problemText(
          problem ?? "failed",
          providerFor(transform, providers, status),
        ),
      });
  };

  if (!transform) return null;

  return (
    <div
      className={
        compact ? "" : "rounded-[var(--r-lg)] border border-line bg-surface p-4"
      }
    >
      <div className="flex flex-wrap items-center gap-2">
        {!compact && (
          <Dropdown
            className="w-[200px]"
            ariaLabel={t("transforms.try.transform")}
            options={transforms.map((x) => ({ value: x.id, label: x.name }))}
            selectedValue={transform.id}
            onSelect={setId}
          />
        )}
        <Button
          onClick={run}
          disabled={!text.trim() || running}
          aria-busy={running}
        >
          <Play className="h-3.5 w-3.5" aria-hidden="true" />
          {running ? t("transforms.try.running") : t("transforms.try.run")}
        </Button>
        <span className="min-w-0 truncate text-[13px] text-text-3">
          {modelLine(transform, providers, status, t)}
        </span>
      </div>

      <div
        className={`mt-4 grid gap-4 ${compact ? "" : "min-[760px]:grid-cols-2"}`}
      >
        <div className="flex min-w-0 flex-col">
          <div className="mb-1.5 flex min-h-6 items-center justify-between gap-2">
            <label htmlFor={textId} className="text-[13px] font-medium">
              {t("transforms.try.text")}
            </label>
            <button
              type="button"
              onClick={loadRecent}
              className="inline-flex items-center gap-1.5 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:text-text hover:duration-0"
            >
              <History className="h-3.5 w-3.5" aria-hidden="true" />
              {t("transforms.try.recent")}
            </button>
          </div>
          <textarea
            id={textId}
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setNoRecent(false);
            }}
            onKeyDown={(e) => {
              if (e.key === "Enter" && (e.ctrlKey || e.metaKey)) {
                e.preventDefault();
                e.stopPropagation();
                run();
              }
            }}
            placeholder={t("transforms.try.placeholder")}
            className="input min-h-[180px] flex-1 resize-none py-2.5 leading-relaxed"
          />
          {noRecent && (
            <p className="mt-1.5 text-[13px] text-text-3">
              {t("transforms.try.noRecent")}
            </p>
          )}
        </div>

        <div className="flex min-w-0 flex-col">
          <div className="mb-1.5 flex min-h-6 items-center justify-between gap-2">
            <span className="text-[13px] font-medium">
              {t("transforms.try.result")}
              {outcome?.kind === "text" && (
                <span className="ms-2 font-normal tabular-nums text-text-3">
                  {formatRunTime(outcome.ms)}
                </span>
              )}
            </span>
            {outcome?.kind === "text" && (
              <div
                role="tablist"
                aria-label={t("transforms.try.view")}
                className="flex rounded-[var(--r-sm)] bg-surface-2 p-0.5"
              >
                {(["changes", "result"] as const).map((v) => (
                  <button
                    key={v}
                    type="button"
                    role="tab"
                    aria-selected={view === v}
                    onClick={() => setView(v)}
                    className={`rounded-[5px] px-2 py-0.5 text-xs transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                      view === v
                        ? "bg-surface font-medium text-text shadow-[var(--shadow-1)]"
                        : "text-text-2 hover:text-text"
                    }`}
                  >
                    {t(`transforms.try.${v}`)}
                  </button>
                ))}
              </div>
            )}
          </div>
          <div
            aria-live="polite"
            className="min-h-[180px] flex-1 overflow-auto rounded-[var(--r-md)] bg-surface-2 px-3 py-2.5 text-sm"
          >
            {!outcome ? (
              <p className="text-text-3">
                {running
                  ? t("transforms.try.running")
                  : t("transforms.try.empty")}
              </p>
            ) : outcome.kind === "problem" ? (
              <p className="text-danger">{outcome.text}</p>
            ) : view === "changes" ? (
              <DiffText before={outcome.before} after={outcome.text} />
            ) : (
              <p className="whitespace-pre-wrap break-words leading-relaxed">
                {outcome.text}
              </p>
            )}
          </div>
        </div>
      </div>
    </div>
  );
};
