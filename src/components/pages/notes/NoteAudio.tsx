import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { convertFileSrc } from "@tauri-apps/api/core";
import { Pause, Play } from "lucide-react";
import { commands, type Note } from "@/bindings";
import { clock, durationLabel } from "../../notes/notes";

/** A note's recording, shared by its tabs: the Transcript marks the line
 *  playing and plays from any line. */
export interface NoteAudio {
  audio: React.RefObject<HTMLAudioElement>;
  src: string | null;
  at: number;
  playing: boolean;
  /** The full player shows once something has played. */
  open: boolean;
  playFrom: (ms: number) => void;
}

export const useNoteAudio = (note: Note): NoteAudio => {
  const audio = useRef<HTMLAudioElement>(null);
  const [src, setSrc] = useState<string | null>(null);
  const [at, setAt] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [open, setOpen] = useState(false);

  useEffect(() => {
    let live = true;
    setSrc(null);
    setOpen(false);
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
  }, [src]);

  const playFrom = (ms: number) => {
    setOpen(true);
    const el = audio.current;
    if (!el) return;
    el.currentTime = ms / 1000;
    el.play().catch(() => {});
  };

  return { audio, src, at, playing, open, playFrom };
};

/** Play recording, then play, a seek bar and the time. Holds the audio
 *  element, so it stays mounted while tabs change. */
export const NotePlayer: React.FC<{ note: Note; player: NoteAudio }> = ({
  note,
  player,
}) => {
  const { t } = useTranslation();
  const { audio, src, at, playing, open, playFrom } = player;
  if (!src) return null;
  const total = Math.max(note.duration_ms, at);
  return (
    <div className="mt-3 flex min-w-0 items-center gap-3">
      {open ? (
        <>
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
            className="slider min-w-0 max-w-[520px] flex-1"
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
        </>
      ) : (
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
      )}
      <audio ref={audio} src={src} preload="metadata" />
    </div>
  );
};
