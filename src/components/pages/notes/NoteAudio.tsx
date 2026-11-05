import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Pause, Play, RotateCcw, RotateCw } from "lucide-react";
import { commands, type Note, type NoteLine } from "@/bindings";
import {
  clock,
  lineAt,
  lineStep,
  nextSpeed,
  playerMarks,
  speakerBands,
} from "../../notes/notes";

/** How far the skip buttons and ← → jump. */
const SKIP_MS = 10_000;
const SPEED_KEY = "legato-note-speed";

const savedSpeed = (): number => {
  try {
    return nextSpeed(Number(localStorage.getItem(SPEED_KEY) ?? 1), 0);
  } catch {
    return 1;
  }
};

/** A note's recording, shared by its tabs: the Transcript marks the line
 *  playing and plays from any line. */
export interface NoteAudio {
  audio: React.RefObject<HTMLAudioElement>;
  src: string | null;
  at: number;
  playing: boolean;
  speed: number;
  /** The transcript, for the seek bar's line under the pointer, the
   *  speaker band and Shift+← →. */
  lines: NoteLine[];
  playFrom: (ms: number) => void;
  /** Moves without starting or stopping playback. */
  seek: (ms: number) => void;
  toggle: () => void;
  setSpeed: (rate: number) => void;
}

export const useNoteAudio = (note: Note): NoteAudio => {
  const audio = useRef<HTMLAudioElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [at, setAt] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [speed, setSpeedState] = useState(savedSpeed);
  const [lines, setLines] = useState<NoteLine[]>([]);

  useEffect(() => {
    let live = true;
    setSrc(null);
    setAt(0);
    setPlaying(false);
    if (!note.audio_file) return;
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

  useEffect(() => {
    let live = true;
    setLines([]);
    if (!note.audio_file || note.status !== "ready") return;
    commands
      .getNoteLines(note.id)
      .then((r) => live && r.status === "ok" && setLines(r.data))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [note.id, note.audio_file, note.status, note.lines]);

  // Follow the audio element.
  useEffect(() => {
    const el = audio.current;
    if (!el) return;
    el.defaultPlaybackRate = speed;
    el.playbackRate = speed;
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
    // Runs per recording; setSpeed applies a new speed itself.
  }, [src]);

  const seek = (ms: number) => {
    const el = audio.current;
    if (!el) return;
    const end = Number.isFinite(el.duration)
      ? el.duration * 1000
      : note.duration_ms;
    const to = Math.max(0, Math.min(ms, end));
    el.currentTime = to / 1000;
    setAt(Math.round(to));
  };

  const playFrom = (ms: number) => {
    seek(ms);
    audio.current?.play().catch(() => {});
  };

  const toggle = () => {
    const el = audio.current;
    if (!el) return;
    if (el.paused) el.play().catch(() => {});
    else el.pause();
  };

  const setSpeed = (rate: number) => {
    setSpeedState(rate);
    if (audio.current) {
      audio.current.defaultPlaybackRate = rate;
      audio.current.playbackRate = rate;
    }
    try {
      localStorage.setItem(SPEED_KEY, String(rate));
    } catch {
      // The speed just isn't remembered.
    }
  };

  return {
    audio,
    src,
    at,
    playing,
    speed,
    lines,
    playFrom,
    seek,
    toggle,
    setSpeed,
  };
};

/** Keys go to the player except while typing, or on a control that uses
 *  that key itself: Space presses buttons, arrows move tabs and menus. */
const typing = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (target.isContentEditable ||
    !!target.closest(
      "input, textarea, select, [role=menu], [role=menuitem], [role=dialog], [role=slider], [role=tab], [role=listbox], [role=option]",
    ));
const takesSpace = (target: EventTarget | null) =>
  typing(target) ||
  (target instanceof HTMLElement &&
    !!target.closest("button, a, [role=button]"));

const formatSpeed = (rate: number) => `${rate}×`;

/** Play, skip back and forward, the seek bar with the summary's topics
 *  marked and who speaks where under it, the time and the speed. Holds the
 *  audio element, so it stays mounted while tabs change.
 *
 *  Keys: Space plays and pauses, ← → skip ten seconds, Shift+← → go to the
 *  previous or next line, < > change the speed. */
export const NotePlayer: React.FC<{ note: Note; player: NoteAudio }> = ({
  note,
  player,
}) => {
  const { t } = useTranslation();
  const { audio, src, at, playing, speed, lines, toggle, seek, setSpeed } =
    player;
  const total = Math.max(note.duration_ms, at, 1);
  const marks = useMemo(
    () => playerMarks(note.outline, total),
    [note.outline, total],
  );
  const bands = useMemo(() => speakerBands(lines), [lines]);
  const track = useRef<HTMLDivElement>(null);
  const [hover, setHover] = useState<{ ms: number; x: number } | null>(null);

  const skip = (ms: number) => seek(at + ms);
  const stepLine = (dir: 1 | -1) => {
    const to = lineStep(lines, at, dir);
    if (to != null) seek(to);
  };

  useEffect(() => {
    if (!src) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.repeat && e.key === " ") return;
      if (e.defaultPrevented || e.ctrlKey || e.altKey || e.metaKey) return;
      if (e.key === " " && !e.shiftKey && !takesSpace(e.target)) {
        e.preventDefault();
        toggle();
      } else if (
        (e.key === "ArrowLeft" || e.key === "ArrowRight") &&
        !typing(e.target)
      ) {
        e.preventDefault();
        const dir = e.key === "ArrowRight" ? 1 : -1;
        if (e.shiftKey) stepLine(dir);
        else skip(dir * SKIP_MS);
      } else if ((e.key === ">" || e.key === "<") && !typing(e.target)) {
        e.preventDefault();
        setSpeed(nextSpeed(speed, e.key === ">" ? 1 : -1));
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  });

  if (!src) return null;
  const pct = (ms: number) => `${Math.min(100, (ms / total) * 100)}%`;
  const hovered = hover
    ? (() => {
        const width = track.current?.clientWidth ?? 1;
        // A mark within six pixels names itself; elsewhere, the line said.
        const near = marks.find(
          (m) => Math.abs(((m.ms - hover.ms) / total) * width) <= 6,
        );
        if (near) return { ms: near.ms, text: near.label, mark: true };
        const line = lines[lineAt(lines, hover.ms)];
        const inside = line && hover.ms <= Math.max(line.end_ms, line.at_ms);
        return { ms: hover.ms, text: inside ? line.text : "", mark: false };
      })()
    : null;

  const iconButton =
    "grid h-8 w-8 flex-none place-items-center rounded-full text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]";

  return (
    <div className="mt-3 flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
      <button
        type="button"
        onClick={toggle}
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
      <button
        type="button"
        onClick={() => skip(-SKIP_MS)}
        aria-label={t("notes.player.back")}
        title={t("notes.player.back")}
        className={`group ${iconButton}`}
      >
        <RotateCcw className="h-4 w-4" aria-hidden="true" />
      </button>
      <button
        type="button"
        onClick={() => skip(SKIP_MS)}
        aria-label={t("notes.player.forward")}
        title={t("notes.player.forward")}
        className={`group ${iconButton}`}
      >
        <RotateCw className="h-4 w-4" aria-hidden="true" />
      </button>
      <div
        ref={track}
        className="relative ms-1 h-8 min-w-[160px] max-w-[560px] flex-1"
        onPointerMove={(e) => {
          const box = e.currentTarget.getBoundingClientRect();
          const x = Math.max(0, Math.min(box.width, e.clientX - box.left));
          setHover({ ms: (x / box.width) * total, x });
        }}
        onPointerLeave={() => setHover(null)}
      >
        <input
          type="range"
          className="slider absolute inset-x-0 top-1/2 w-full -translate-y-1/2"
          style={{ "--pct": pct(at) } as React.CSSProperties}
          min={0}
          max={total}
          step={250}
          value={at}
          aria-label={t("notes.transcript.seek")}
          aria-valuetext={clock(at)}
          onChange={(e) => seek(Number(e.target.value))}
          onKeyDown={(e) => {
            // Arrows here skip as they do anywhere else on the page.
            if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
            e.preventDefault();
            const dir = e.key === "ArrowRight" ? 1 : -1;
            if (e.shiftKey) stepLine(dir);
            else skip(dir * SKIP_MS);
          }}
        />
        {marks.map((m) => (
          <span
            key={m.ms}
            aria-hidden="true"
            className="pointer-events-none absolute top-1/2 h-2.5 w-0.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-text-2"
            style={{ left: pct(m.ms) }}
          />
        ))}
        {bands.length > 0 && (
          <div
            aria-hidden="true"
            className="pointer-events-none absolute inset-x-0 bottom-0.5 h-[3px] overflow-hidden rounded-full bg-surface-3"
          >
            {bands.map((b) => (
              <span
                key={b.from}
                className="absolute inset-y-0"
                style={{
                  left: pct(b.from),
                  width: `max(1px, calc(${pct(b.to)} - ${pct(b.from)}))`,
                  background: b.colour,
                }}
              />
            ))}
          </div>
        )}
        {hovered && hover && (
          <div
            role="presentation"
            className="pointer-events-none absolute bottom-full z-20 mb-1 max-w-[280px] -translate-x-1/2 rounded-[var(--r-sm)] border border-line bg-surface px-2 py-1 text-[12px] shadow-[var(--shadow-2)]"
            style={{
              left: `clamp(70px, ${hover.x}px, calc(100% - 70px))`,
            }}
          >
            <span className="font-medium tabular-nums">
              {clock(hovered.ms)}
            </span>
            {hovered.text && (
              <span
                className={`ms-1.5 line-clamp-2 inline ${hovered.mark ? "font-medium text-text" : "text-text-2"}`}
              >
                {hovered.text}
              </span>
            )}
          </div>
        )}
      </div>
      <span className="flex-none text-xs tabular-nums text-text-3">
        {clock(at)} / {clock(total)}
      </span>
      <button
        type="button"
        onClick={() => setSpeed(nextSpeed(speed, 1, true))}
        aria-label={t("notes.player.speed", { speed: formatSpeed(speed) })}
        title={t("notes.player.speed", { speed: formatSpeed(speed) })}
        className="h-7 min-w-[44px] flex-none rounded-[var(--r-sm)] px-1.5 text-xs font-medium tabular-nums text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:text-text hover:duration-0 active:scale-[0.97]"
      >
        {formatSpeed(speed)}
      </button>
      <audio ref={audio} src={src} preload="metadata" />
    </div>
  );
};
