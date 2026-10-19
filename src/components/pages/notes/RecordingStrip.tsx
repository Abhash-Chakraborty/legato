import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  ChevronDown,
  ChevronUp,
  Loader2,
  Pause,
  Play,
  Square,
} from "lucide-react";
import { commands, type NoteLine, type SessionInfo } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useNotesStore } from "../../../stores/notesStore";
import { Button } from "../../ui/Button";
import { clock, longClock } from "../../notes/notes";

const BARS = 8;

/** Eight bars for one side of the recording; flat when it is silent. */
export const SideMeter: React.FC<{ level: number; label: string }> = ({
  level,
  label,
}) => {
  const lit = Math.round(Math.min(1, Math.max(0, level)) * BARS);
  return (
    <span className="inline-flex items-center gap-[7px] text-xs text-text-3">
      {label}
      <span className="flex h-3.5 items-end gap-[2px]" aria-hidden="true">
        {Array.from({ length: BARS }, (_, i) => (
          <span
            key={i}
            className={`h-3.5 w-[3px] rounded-full transition-colors duration-[70ms] ${
              i < lit ? "bg-teal-600" : "bg-line-strong"
            }`}
          />
        ))}
      </span>
    </span>
  );
};

const LiveLine: React.FC<{ line: NoteLine }> = ({ line }) => {
  const { t } = useTranslation();
  return (
    <li className="grid grid-cols-[44px_48px_minmax(0,1fr)] items-baseline gap-x-2 py-[3px] text-sm">
      <span className="text-xs tabular-nums text-text-3">
        {clock(line.at_ms)}
      </span>
      <span
        className={`text-xs font-medium ${line.who === "you" ? "text-teal-700" : "text-text-2"}`}
      >
        {t(`notes.who.${line.who}`)}
      </span>
      <span className="min-w-0">{line.text}</span>
    </li>
  );
};

/** The recording in progress: its clock, a meter per side, Pause and Stop,
 *  and the lines transcribed so far. */
export const RecordingStrip: React.FC<{ session: SessionInfo }> = ({
  session,
}) => {
  const { t } = useTranslation();
  const tick = useNotesStore((s) => s.tick);
  const live = useNotesStore((s) => s.live);
  const { settings, updateSetting } = useSettings();
  const [busy, setBusy] = useState(false);
  const listRef = useRef<HTMLOListElement>(null);
  const [pinned, setPinned] = useState(true);
  const showLive = settings?.notes_live_transcript ?? true;
  const finishing = session.phase === "finishing";
  const paused = tick?.paused ?? session.paused;
  const elapsed = tick?.elapsed_ms ?? session.elapsed_ms;
  const pending = tick?.pending ?? session.pending;

  // Follow new lines unless you scrolled up to read.
  useEffect(() => {
    const list = listRef.current;
    if (list && pinned) list.scrollTop = list.scrollHeight;
  }, [live.length, pinned, showLive]);

  const run = async (fn: () => Promise<{ status: string; error?: string }>) => {
    if (busy) return;
    setBusy(true);
    const result = await fn().catch((e) => ({
      status: "error",
      error: String(e),
    }));
    setBusy(false);
    if (result.status === "error")
      toast.error(t("notes.toast.actionFailed"), {
        description: result.error,
      });
  };

  const kind = t(`notes.kind.${session.kind}`);
  const status = finishing
    ? t("notes.strip.finishing")
    : paused
      ? t("notes.strip.paused")
      : t("notes.strip.recording");

  return (
    <section
      aria-label={t("notes.strip.label")}
      className={`mb-4 overflow-hidden rounded-[var(--r-lg)] border bg-surface ${
        finishing
          ? "border-line"
          : "border-[color-mix(in_srgb,var(--danger)_35%,var(--border))]"
      }`}
    >
      <div className="flex min-h-14 flex-wrap items-center gap-x-3 gap-y-2 py-2 pe-3 ps-[18px]">
        <span className="inline-flex flex-none items-center gap-2 whitespace-nowrap text-sm font-medium">
          {finishing ? (
            <Loader2
              className="h-4 w-4 animate-spin text-teal-700"
              aria-hidden="true"
            />
          ) : (
            <span
              aria-hidden="true"
              className={`h-2.5 w-2.5 rounded-full ${paused ? "bg-warn" : "notes-recdot bg-danger"}`}
            />
          )}
          <span role="status">{status}</span>
          {!finishing && (
            <span className="tabular-nums">· {longClock(elapsed)}</span>
          )}
          <span className="font-normal text-text-2">· {kind}</span>
        </span>
        {finishing ? (
          <span className="min-w-0 flex-1 truncate text-[13px] text-text-3">
            {pending
              ? t("notes.strip.linesLeft", { count: pending })
              : t("notes.strip.saving")}
          </span>
        ) : (
          <span className="flex min-w-0 flex-1 items-center gap-3.5 ps-1.5">
            <SideMeter
              level={paused ? 0 : (tick?.you ?? 0)}
              label={t("notes.who.you")}
            />
            {session.kind === "meeting" && (
              <SideMeter
                level={paused ? 0 : (tick?.them ?? 0)}
                label={t("notes.who.them")}
              />
            )}
            {session.system_fallback && (
              <span className="truncate text-xs text-text-3">
                {t("notes.strip.fallback")}
              </span>
            )}
          </span>
        )}
        {!finishing && (
          <div className="flex flex-none items-center gap-1.5">
            <Button
              variant="secondary"
              size="sm"
              disabled={busy}
              onClick={() => run(() => commands.pauseNotes(!paused))}
            >
              {paused ? (
                <Play className="h-4 w-4" aria-hidden="true" />
              ) : (
                <Pause className="h-4 w-4" aria-hidden="true" />
              )}
              {paused ? t("notes.strip.resume") : t("notes.strip.pause")}
            </Button>
            <Button
              size="sm"
              disabled={busy}
              onClick={() => run(() => commands.stopNotes())}
            >
              <Square className="h-3.5 w-3.5" aria-hidden="true" />
              {t("notes.strip.stop")}
            </Button>
          </div>
        )}
      </div>
      {!finishing && (
        <div className="border-t border-line">
          <div className="flex min-h-10 items-center gap-2 py-1 pe-2 ps-[18px] text-xs text-text-3">
            <span className="font-semibold text-text-2">
              {t("notes.strip.live")}
            </span>
            <span className="flex-1" />
            {showLive && live.length > 0 && (
              <span className="tabular-nums">
                {t("notes.strip.lines", { count: live.length })}
              </span>
            )}
            <button
              type="button"
              onClick={() => updateSetting("notes_live_transcript", !showLive)}
              aria-expanded={showLive}
              className="inline-flex h-7 items-center gap-1 rounded-[var(--r-sm)] px-2 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0"
            >
              {showLive ? (
                <ChevronUp className="h-3.5 w-3.5" aria-hidden="true" />
              ) : (
                <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
              )}
              {showLive ? t("notes.strip.hide") : t("notes.strip.show")}
            </button>
          </div>
          {showLive && (
            <div className="relative">
              {live.length ? (
                <ol
                  ref={listRef}
                  aria-live="polite"
                  aria-label={t("notes.strip.live")}
                  onScroll={(e) => {
                    const el = e.currentTarget;
                    setPinned(
                      el.scrollHeight - el.scrollTop - el.clientHeight < 24,
                    );
                  }}
                  className="max-h-[132px] overflow-y-auto pb-2.5 pe-4 ps-[18px]"
                >
                  {live.map((line) => (
                    <LiveLine key={line.id} line={line} />
                  ))}
                </ol>
              ) : (
                <p className="pb-3 ps-[18px] text-[13px] text-text-3">
                  {pending
                    ? t("notes.strip.transcribing")
                    : t("notes.strip.waiting")}
                </p>
              )}
              {!pinned && (
                <button
                  type="button"
                  onClick={() => setPinned(true)}
                  className="absolute bottom-2 end-4 rounded-full bg-surface-3 px-2.5 py-1 text-xs font-medium text-text-2 shadow-sm hover:text-text"
                >
                  {t("notes.strip.latest")}
                </button>
              )}
            </div>
          )}
        </div>
      )}
    </section>
  );
};
