import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Copy,
  Loader2,
  MoreHorizontal,
  RotateCw,
  Search,
  SpellCheck,
  Users,
  X,
} from "lucide-react";
import { toast } from "sonner";
import { commands, type Note, type NoteLine } from "@/bindings";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import { useSettings } from "../../../hooks/useSettings";
import { useSpeakerModel } from "../../../hooks/useSpeakerModel";
import { useAiStore, watchAi } from "../../../stores/aiStore";
import { useNavStore } from "../../../stores/navStore";
import { useNotesStore } from "../../../stores/notesStore";
import { ActionMenu } from "../../ui/ActionMenu";
import { Button } from "../../ui/Button";
import { LineText } from "../../notes/LineText";
import { clock, lineAt, lineMatches, voiceChoices } from "../../notes/notes";
import { SpeakerTag } from "../../notes/SpeakerTag";
import { FixReview } from "./FixReview";
import type { NoteAudio } from "./NoteAudio";
import type { Jump } from "./NoteDetail";

/** The transcript, line by line with who spoke and when; plays the
 *  recording from any line, and searches it. */
export const TranscriptTab: React.FC<{
  note: Note;
  jump: Jump | null;
  onCopy: () => void;
  player: NoteAudio;
}> = ({ note, jump, onCopy, player }) => {
  const { t } = useTranslation();
  const linesVersion = useNotesStore((s) => s.linesVersion);
  const [lines, setLines] = useState<NoteLine[] | null>(null);
  const [query, setQuery] = useState("");
  const [retrying, setRetrying] = useState(false);
  const { src, at, playing, playFrom } = player;
  const [marked, setMarked] = useState<number | null>(null);
  const [reviewing, setReviewing] = useState(false);
  const [finding, setFinding] = useState(false);
  const [menu, setMenu] = useState(false);
  const [searching, setSearching] = useState(false);
  const menuButton = useRef<HTMLButtonElement>(null);
  const searchBox = useRef<HTMLInputElement>(null);
  const openSearch = () => {
    setSearching(true);
    requestAnimationFrame(() => searchBox.current?.focus());
  };
  const speakerModel = useSpeakerModel();
  const go = useNavStore((s) => s.go);
  const aiStatus = useAiStore((s) => s.status);
  const { settings } = useSettings();
  const canFix =
    (settings?.ai_notes_enabled ?? true) &&
    !!aiStatus &&
    aiStatus.kind !== "none" &&
    note.status === "ready";
  const listRef = useRef<HTMLOListElement>(null);

  useEffect(watchAi, []);
  useEffect(() => setReviewing(false), [note.id]);

  useEffect(() => {
    let live = true;
    commands
      .getNoteLines(note.id)
      .then((r) => live && r.status === "ok" && setLines(r.data))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [note.id, linesVersion]);

  const voices = useMemo(() => voiceChoices(lines ?? []), [lines]);
  const canFindSpeakers =
    !!speakerModel?.downloaded && !!note.audio_file && note.status === "ready";
  // A meeting with no voices told apart: say where that's set up.
  const offerSpeakers =
    !!speakerModel &&
    !speakerModel.downloaded &&
    note.kind === "meeting" &&
    note.status === "ready" &&
    !!note.audio_file;

  const findSpeakers = async () => {
    setFinding(true);
    const result = await commands.findNoteSpeakers(note.id);
    setFinding(false);
    if (result.status === "error")
      toast.error(t("notes.speakers.findFailed"), {
        description: result.error,
      });
    else if (result.data < 2) toast(t("notes.speakers.oneVoice"));
    else toast(t("notes.speakers.found", { count: result.data }));
  };

  const shown = useMemo(
    () => (lines ?? []).filter((l) => !query.trim() || lineMatches(l, query)),
    [lines, query],
  );
  const current = playing || at > 0 ? lineAt(lines ?? [], at) : -1;
  const currentId = current >= 0 ? lines?.[current]?.id : undefined;

  useTextHighlight(
    listRef,
    "transcript-search",
    query.trim() ? query.trim().split(/\s+/) : [],
    "[data-line-text]",
  );

  const scrollTo = (id: number) =>
    listRef.current
      ?.querySelector<HTMLElement>(`[data-line="${id}"]`)
      ?.scrollIntoView({ block: "center", behavior: "smooth" });

  // An Ask answer's time: mark that line and bring it into view.
  useEffect(() => {
    if (!jump || !lines) return;
    setQuery("");
    const i = lineAt(lines, jump.ms + 999);
    const line = lines[Math.max(0, i)];
    if (!line) return;
    setMarked(line.id);
    requestAnimationFrame(() => scrollTo(line.id));
  }, [jump?.n, lines === null]);

  if (lines && lines.length === 0)
    return (
      <p className="text-sm text-text-2">
        {note.status === "ready"
          ? t("notes.transcript.empty")
          : t("notes.transcript.waiting")}
      </p>
    );

  const retry = async () => {
    setRetrying(true);
    const result = await commands.retryNoteLines(note.id);
    setRetrying(false);
    if (result.status === "error") {
      toast.error(t("notes.transcript.retryFailed"), {
        description: result.error,
      });
    } else if (result.data.failed > 0) {
      toast.warning(
        t("notes.transcript.retryPartly", { count: result.data.failed }),
      );
    } else {
      toast(t("notes.transcript.retried", { count: result.data.transcribed }));
    }
  };
  return (
    <div>
      <div className="mb-4 flex flex-wrap items-center gap-2">
        {!src && note.status === "ready" && (
          <span className="text-[13px] text-text-3">
            {t("notes.transcript.noAudio")}
          </span>
        )}
        <span className="flex-1" />
        {finding && (
          <span
            role="status"
            className="inline-flex items-center gap-2 text-[13px] text-text-2"
          >
            <Loader2
              className="h-3.5 w-3.5 animate-spin motion-reduce:animate-none"
              aria-hidden="true"
            />
            {t("notes.speakers.finding")}
          </span>
        )}
        {searching || query ? (
          <div className="relative w-[220px]">
            <Search
              className="pointer-events-none absolute start-[10px] top-1/2 h-[14px] w-[14px] -translate-y-1/2 text-text-3"
              aria-hidden="true"
            />
            <input
              ref={searchBox}
              type="search"
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              onBlur={() => !query && setSearching(false)}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  e.preventDefault();
                  e.stopPropagation();
                  setQuery("");
                  setSearching(false);
                }
              }}
              placeholder={t("notes.transcript.search")}
              aria-label={t("notes.transcript.search")}
              className="input compact w-full pe-8 ps-[30px] [&::-webkit-search-cancel-button]:hidden"
            />
            <button
              type="button"
              onMouseDown={(e) => e.preventDefault()}
              onClick={() => {
                setQuery("");
                setSearching(false);
              }}
              aria-label={t("notes.transcript.clearSearch")}
              title={t("notes.transcript.clearSearch")}
              className="absolute end-1 top-1/2 grid h-6 w-6 -translate-y-1/2 place-items-center rounded-[var(--r-xs)] text-text-3 hover:bg-surface-3 hover:text-text"
            >
              <X className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
          </div>
        ) : (
          <button
            type="button"
            onClick={openSearch}
            aria-label={t("notes.transcript.search")}
            title={t("notes.transcript.search")}
            className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
          >
            <Search className="h-4 w-4" aria-hidden="true" />
          </button>
        )}
        <button
          type="button"
          onClick={onCopy}
          aria-label={t("notes.transcript.copy")}
          title={t("notes.transcript.copy")}
          className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
        >
          <Copy className="h-4 w-4" aria-hidden="true" />
        </button>
        {(offerSpeakers || canFindSpeakers || canFix) && (
          <div className="relative">
            <button
              ref={menuButton}
              type="button"
              onClick={() => setMenu(!menu)}
              aria-haspopup="menu"
              aria-expanded={menu}
              aria-label={t("notes.transcript.more")}
              title={t("notes.transcript.more")}
              className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
            >
              <MoreHorizontal className="h-4 w-4" aria-hidden="true" />
            </button>
            {menu && (
              <ActionMenu
                label={t("notes.transcript.more")}
                className="end-0 top-full z-10 mt-1 w-60"
                onClose={(restore) => {
                  setMenu(false);
                  if (restore) menuButton.current?.focus();
                }}
                items={[
                  ...(canFix
                    ? [
                        {
                          id: "misheard",
                          label: t("notes.aiFix.find"),
                          icon: SpellCheck,
                          disabled: reviewing,
                          onSelect: () => setReviewing(true),
                        },
                      ]
                    : []),
                  ...(canFindSpeakers
                    ? [
                        {
                          id: "speakers",
                          label: t("notes.speakers.findAgain"),
                          icon: Users,
                          disabled: finding,
                          onSelect: findSpeakers,
                        },
                      ]
                    : []),
                  ...(offerSpeakers
                    ? [
                        {
                          id: "setup",
                          label: t("notes.speakers.setUp"),
                          icon: Users,
                          onSelect: () => go("settings", "calls"),
                        },
                      ]
                    : []),
                ]}
              />
            )}
          </div>
        )}
      </div>

      {reviewing && (
        <FixReview
          note={note}
          onJump={(id) => {
            setQuery("");
            setMarked(id);
            requestAnimationFrame(() => scrollTo(id));
          }}
          onClose={() => setReviewing(false)}
        />
      )}

      {note.failed_lines > 0 && (
        <div className="mb-3 flex flex-wrap items-center gap-x-3 gap-y-1">
          <p className="text-[13px] text-warn">
            {t("notes.transcript.failed", { count: note.failed_lines })}
          </p>
          {(note.retryable ?? 0) > 0 && note.audio_file && (
            <Button
              variant="ghost"
              size="sm"
              disabled={retrying}
              onClick={retry}
            >
              <RotateCw
                className={`h-3.5 w-3.5 ${retrying ? "animate-spin motion-reduce:animate-none" : ""}`}
                aria-hidden="true"
              />
              {retrying
                ? t("notes.transcript.retrying")
                : t("notes.transcript.retry")}
            </Button>
          )}
        </div>
      )}

      {query.trim() && shown.length === 0 ? (
        <div className="grid place-items-center gap-1.5 py-10 text-center">
          <p className="text-[15px] font-medium">
            {t("notes.transcript.noResults", { query: query.trim() })}
          </p>
          <button
            type="button"
            onClick={() => setQuery("")}
            className="text-sm font-medium text-teal-700 underline-offset-2 hover:underline"
          >
            {t("notes.clearSearch")}
          </button>
        </div>
      ) : (
        <ol ref={listRef} aria-label={t("notes.detail.tab.transcript")}>
          {shown.map((line) => {
            const now = line.id === currentId;
            const mark = line.id === marked;
            return (
              <li
                key={line.id}
                data-line={line.id}
                className={`grid ${voices.length ? "grid-cols-[52px_96px_minmax(0,1fr)]" : "grid-cols-[52px_52px_minmax(0,1fr)]"} items-baseline gap-x-2 rounded-[var(--r-sm)] px-1.5 py-[7px] text-[15px] leading-relaxed transition-colors duration-[var(--dur-2)] ${
                  now ? "bg-teal-50" : mark ? "bg-surface-2" : ""
                }`}
              >
                <button
                  type="button"
                  disabled={!src}
                  onClick={() => playFrom(line.at_ms)}
                  title={
                    src
                      ? t("notes.transcript.playFrom", {
                          time: clock(line.at_ms),
                        })
                      : undefined
                  }
                  className="justify-self-start rounded-[var(--r-xs)] px-1 text-xs tabular-nums text-text-3 enabled:hover:bg-surface-3 enabled:hover:text-text disabled:cursor-default"
                >
                  {clock(line.at_ms)}
                </button>
                <SpeakerTag line={line} className="text-[13px]" />
                <LineText line={line} voices={voices} />
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
};
