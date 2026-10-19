import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Copy, Pause, Play, Search } from "lucide-react";
import { commands, type Note, type NoteLine } from "@/bindings";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import { useNotesStore } from "../../../stores/notesStore";
import { clock, durationLabel, lineAt, lineMatches } from "../../notes/notes";
import type { Jump } from "./NoteDetail";

/** Play, a seek bar and the time; the line playing is marked below. */
const Player: React.FC<{
  audio: React.RefObject<HTMLAudioElement>;
  src: string;
  durationMs: number;
  at: number;
  playing: boolean;
}> = ({ audio, src, durationMs, at, playing }) => {
  const { t } = useTranslation();
  const total = Math.max(durationMs, at);
  return (
    <div className="flex min-w-0 flex-1 items-center gap-3">
      <button
        type="button"
        onClick={() => {
          const el = audio.current;
          if (!el) return;
          if (el.paused) el.play().catch(() => {});
          else el.pause();
        }}
        aria-label={
          playing ? t("notes.transcript.pause") : t("notes.transcript.play")
        }
        title={
          playing ? t("notes.transcript.pause") : t("notes.transcript.play")
        }
        className="grid h-8 w-8 flex-none place-items-center rounded-full bg-teal-fill text-white transition-colors duration-[var(--dur-1)] hover:bg-teal-fill-hover active:scale-[0.97]"
      >
        {playing ? (
          <Pause className="h-3.5 w-3.5" aria-hidden="true" />
        ) : (
          <Play className="ms-px h-3.5 w-3.5" aria-hidden="true" />
        )}
      </button>
      <input
        type="range"
        className="slider min-w-0 flex-1"
        min={0}
        max={total}
        step={250}
        value={at}
        aria-label={t("notes.transcript.seek")}
        aria-valuetext={clock(at)}
        onChange={(e) => {
          if (audio.current)
            audio.current.currentTime = Number(e.target.value) / 1000;
        }}
      />
      <span className="flex-none text-xs tabular-nums text-text-3">
        {clock(at)} / {clock(total)}
      </span>
      <audio ref={audio} src={src} preload="metadata" />
    </div>
  );
};

/** The transcript, line by line with who spoke and when; plays the
 *  recording from any line, and searches it. */
export const TranscriptTab: React.FC<{
  note: Note;
  jump: Jump | null;
  onCopy: () => void;
}> = ({ note, jump, onCopy }) => {
  const { t } = useTranslation();
  const linesVersion = useNotesStore((s) => s.linesVersion);
  const [lines, setLines] = useState<NoteLine[] | null>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  const [at, setAt] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [player, setPlayer] = useState(false);
  const [marked, setMarked] = useState<number | null>(null);
  const audio = useRef<HTMLAudioElement>(null);
  const listRef = useRef<HTMLOListElement>(null);

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

  useEffect(() => {
    let live = true;
    setSrc(null);
    setPlayer(false);
    commands
      .getNoteAudio(note.id)
      .then(
        (r) =>
          live &&
          r.status === "ok" &&
          r.data &&
          setSrc(convertFileSrc(r.data, "asset")),
      )
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [note.id, note.audio_file]);

  // Follow the audio element.
  useEffect(() => {
    const el = audio.current;
    if (!el) return;
    const time = () => setAt(Math.round(el.currentTime * 1000));
    const play = () => setPlaying(true);
    const pause = () => setPlaying(false);
    el.addEventListener("timeupdate", time);
    el.addEventListener("seeked", time);
    el.addEventListener("play", play);
    el.addEventListener("pause", pause);
    el.addEventListener("ended", pause);
    return () => {
      el.removeEventListener("timeupdate", time);
      el.removeEventListener("seeked", time);
      el.removeEventListener("play", play);
      el.removeEventListener("pause", pause);
      el.removeEventListener("ended", pause);
    };
  }, [player, src]);

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

  const playFrom = (ms: number) => {
    setPlayer(true);
    requestAnimationFrame(() => {
      const el = audio.current;
      if (!el) return;
      el.currentTime = ms / 1000;
      el.play().catch(() => {});
    });
  };

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

  return (
    <div>
      <div className="mb-4 flex flex-wrap items-center gap-2">
        {src && player ? (
          <Player
            audio={audio}
            src={src}
            durationMs={note.duration_ms}
            at={at}
            playing={playing}
          />
        ) : src ? (
          <button
            type="button"
            onClick={() => playFrom(0)}
            className="inline-flex h-8 items-center gap-2 rounded-[var(--r-md)] border border-line px-3 text-[13px] font-medium transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0 active:scale-[0.97]"
          >
            <Play className="h-3.5 w-3.5" aria-hidden="true" />
            {t("notes.transcript.playRecording")}
            <span className="font-normal text-text-3">
              · {durationLabel(note.duration_ms)}
            </span>
          </button>
        ) : (
          <span className="flex-1 text-[13px] text-text-3">
            {note.status === "ready" ? t("notes.transcript.noAudio") : ""}
          </span>
        )}
        <span className={src && player ? "" : "flex-1"} />
        <div className="relative w-[200px]">
          <Search
            className="pointer-events-none absolute start-[10px] top-1/2 h-[14px] w-[14px] -translate-y-1/2 text-text-3"
            aria-hidden="true"
          />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape" && query) {
                e.preventDefault();
                e.stopPropagation();
                setQuery("");
              }
            }}
            placeholder={t("notes.transcript.search")}
            aria-label={t("notes.transcript.search")}
            className="input compact w-full ps-[30px] [&::-webkit-search-cancel-button]:hidden"
          />
        </div>
        <button
          type="button"
          onClick={onCopy}
          aria-label={t("notes.transcript.copy")}
          title={t("notes.transcript.copy")}
          className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
        >
          <Copy className="h-4 w-4" aria-hidden="true" />
        </button>
      </div>

      {note.failed_lines > 0 && (
        <p className="mb-3 text-[13px] text-warn">
          {t("notes.transcript.failed", { count: note.failed_lines })}
        </p>
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
                className={`grid grid-cols-[52px_52px_minmax(0,1fr)] items-baseline gap-x-2 rounded-[var(--r-sm)] px-1.5 py-[7px] text-[15px] leading-relaxed transition-colors duration-[var(--dur-2)] ${
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
                <span
                  className={`text-[13px] font-medium ${line.who === "you" ? "text-teal-700" : "text-text-2"}`}
                >
                  {t(`notes.who.${line.who}`)}
                </span>
                <span data-line-text className="min-w-0">
                  {line.text}
                </span>
              </li>
            );
          })}
        </ol>
      )}
    </div>
  );
};
