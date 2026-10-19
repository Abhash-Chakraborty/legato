import { listen } from "@tauri-apps/api/event";
import React, { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import "./RecordingOverlay.css";
import { commands, events } from "@/bindings";
import type {
  StreamPhase,
  StreamPhaseEvent,
  StreamTextEvent,
  StreamWorkKind,
  NotesTick,
} from "@/bindings";
import { clock, durationLabel } from "@/components/notes/notes";
import type { ModelStateEvent } from "@/lib/types/events";
import { wordDiff } from "@/lib/utils/diff";
import { formatKeyCombination } from "@/lib/utils/keyboard";

type OverlayState =
  | "recording"
  | "streaming"
  | "transcribing"
  | "processing"
  | "notice"
  | "changes"
  | "pick"
  | "notes"
  | "call";

type AiProblemKind =
  | "none"
  | "unreachable"
  | "rejected"
  | "not_found"
  | "rate_limited"
  | "slow"
  | "failed";

/** A one-line message, such as a word added with the selection shortcut. */
type OverlayNotice =
  | { kind: "word_added"; word: string }
  | { kind: "word_exists"; word: string }
  | { kind: "no_selection" | "not_a_word" | "failed" }
  | {
      kind: "ai_skipped";
      problem: AiProblemKind;
      provider_id: string;
      provider_label: string;
    }
  | { kind: "pasted"; words: number; app: string | null; undo: boolean }
  | { kind: "transformed"; name: string; words: number; undo: boolean }
  | { kind: "copied"; name: string }
  | { kind: "from_phone"; name: string; image: boolean }
  | { kind: "pasted_last" | "copied_last"; words: number }
  | { kind: "undone"; clipboard: boolean }
  | {
      kind:
        | "nothing_to_transform"
        | "no_last_dictation"
        | "no_changes"
        | "ask_empty";
    }
  | {
      kind: "saved_to_scratchpad";
      app: string | null;
      admin: boolean;
      note_id: number | null;
    }
  | {
      kind: "spotted" | "learned";
      word: string;
      more: number;
      app: string | null;
    }
  | { kind: "note_ready"; id: number; duration_ms: number }
  | { kind: "note_empty"; duration_ms: number }
  | { kind: "notes_failed"; reason: string }
  | { kind: "call_never"; app: string }
  | {
      kind: "transform_failed";
      name: string;
      problem: AiProblemKind;
      provider_id: string;
      provider_label: string;
    };

/** The call card: notes offered for a call, or the call ended. */
type CallCard =
  | {
      kind: "offer";
      app: string;
      name: string;
      app_path: string | null;
      countdown_ms: number | null;
      test: boolean;
    }
  | {
      kind: "ended";
      app: string;
      name: string;
      app_path: string | null;
      stop_in_ms: number | null;
      elapsed_ms: number;
    };

/** The call app's own icon, or a phone. */
const CallIcon: React.FC<{ path: string | null }> = ({ path }) => {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    setSrc(null);
    if (path)
      commands
        .getAppIcon(path)
        .then((icon) => live && setSrc(icon))
        .catch(() => {});
    return () => {
      live = false;
    };
  }, [path]);
  return src ? (
    <img className="scall-icon" src={src} alt="" aria-hidden="true" />
  ) : (
    <svg viewBox="0 0 16 16" aria-hidden="true" className="scall-icon glyph">
      <path
        d="M5.2 2.5 6.6 5.6 5.3 6.7a7.6 7.6 0 0 0 4 4l1.1-1.3 3.1 1.4-.5 2.2c-5.6.4-10.4-4.4-10-10Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.3"
        strokeLinejoin="round"
      />
    </svg>
  );
};

/** A transform's changes, shown with Keep and Undo. */
interface BarChanges {
  name: string;
  before: string;
  after: string;
  keys: string;
}

interface PickItem {
  id: string;
  name: string;
  keys: string;
}

const GOOD_NOTICES = new Set([
  "word_added",
  "pasted",
  "transformed",
  "copied",
  "from_phone",
  "pasted_last",
  "copied_last",
  "undone",
  "learned",
  "note_ready",
]);
const WARN_NOTICES = new Set([
  "failed",
  "ai_skipped",
  "transform_failed",
  "saved_to_scratchpad",
  "notes_failed",
]);

const NOTES_BARS = 5;

/** One side of a Notes recording, as five bars. */
const NotesMeter: React.FC<{ level: number }> = ({ level }) => {
  const lit = Math.round(Math.min(1, Math.max(0, level)) * NOTES_BARS);
  return (
    <span className="snotes-meter">
      {Array.from({ length: NOTES_BARS }, (_, i) => (
        <i key={i} className={i < lit ? "on" : ""} />
      ))}
    </span>
  );
};

const CheckMark = () => (
  <svg viewBox="0 0 16 16" aria-hidden="true" className="snotice-ic ok">
    <path
      d="M3.5 8.5 L6.5 11.5 L12.5 4.5"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.8"
      strokeLinecap="round"
      strokeLinejoin="round"
    />
  </svg>
);

const WarnMark = () => (
  <svg viewBox="0 0 16 16" aria-hidden="true" className="snotice-ic warn">
    <path
      d="M8 2.5 L14 13 H2 Z"
      fill="none"
      stroke="currentColor"
      strokeWidth="1.5"
      strokeLinejoin="round"
    />
    <path
      d="M8 6.5 V9.5 M8 11.4 V11.5"
      stroke="currentColor"
      strokeWidth="1.6"
      strokeLinecap="round"
    />
  </svg>
);

// Number of reactive bars in the waveform (the simple, smoothed style shared by
// every overlay form). Mic levels arrive as 16 FFT buckets; we take the first N.
const WAVE_BARS = 9;

// Only call out a model load in the Live preview once it has run this long.
// Warm loads finish in well under this (~1.5s on a Windows CPU backend), so the
// common case never flashes a loading notice while the user is speaking; cold
// loads can take 40s+.
const SLOW_MODEL_LOAD_MS = 2000;

const RecordingOverlay: React.FC = () => {
  const { t } = useTranslation();
  const [isVisible, setIsVisible] = useState(false);
  const [state, setState] = useState<OverlayState>("recording");
  const [notice, setNotice] = useState<OverlayNotice | null>(null);
  const [changes, setChanges] = useState<BarChanges | null>(null);
  const [picks, setPicks] = useState<PickItem[]>([]);
  // The row the arrow keys are on; Enter runs it.
  const [pickIndex, setPickIndex] = useState(0);
  const [notesTick, setNotesTick] = useState<NotesTick | null>(null);
  const [callCard, setCallCard] = useState<{
    card: CallCard;
    at: number;
  } | null>(null);
  const [now, setNow] = useState(Date.now());
  // "Polish…" while a transform runs; the default label otherwise.
  const [workingLabel, setWorkingLabel] = useState<string | null>(null);
  // `Stream::play()` returning does not mean hardware callbacks are flowing.
  // Stay visually in an arming state until the backend processes the first
  // actual microphone sample chunk.
  const [captureReady, setCaptureReady] = useState(false);
  // Recording starts while the model loads in the background. Say so, otherwise
  // a cold start looks like an empty Live preview that ignores speech, then a
  // hung "Transcribing..." spinner. Once recording stops, any in-flight load is
  // the wait, so the working label reflects it immediately; the Live notice
  // waits for the load to be slow so fast loads don't flash it mid-speech.
  const [modelLoading, setModelLoading] = useState(false);
  const [modelLoadSlow, setModelLoadSlow] = useState(false);
  const modelLoadTimerRef = useRef<ReturnType<typeof setTimeout>>();
  // Latched per Live session: once the loading notice has opened the panel, keep
  // it open after the load so it doesn't collapse and reopen when text arrives.
  const [loadNoticeShown, setLoadNoticeShown] = useState(false);
  const [levels, setLevels] = useState<number[]>(Array(WAVE_BARS).fill(0));
  const [streamText, setStreamText] = useState<StreamTextEvent>({
    committed: "",
    tentative: "",
  });
  const [phase, setPhase] = useState<StreamPhase>("listening");
  const [workKind, setWorkKind] = useState<StreamWorkKind>("transcribing");
  const [elapsed, setElapsed] = useState(0);
  // Bumped on each new streaming session so the Live card remounts fresh (replays
  // the pop-in, and never animates in from the previous panel's open size).
  const [session, setSession] = useState(0);
  // Overlay placement (top vs bottom of the screen). The Live panel grows downward
  // from a top overlay (oldest line under the pill) and upward from a bottom one.
  const [position, setPosition] = useState<"top" | "bottom">("bottom");
  // True once live text overflows the cap. A top overlay fades its top edge only
  // while overflowing, so the resting first line stays crisp flush under the pill.
  const [overflowing, setOverflowing] = useState(false);

  const smoothedLevelsRef = useRef<number[]>(Array(16).fill(0));
  // Live-text scroll-back: the text region "sticks" to the newest line while the
  // user is at the bottom; if they scroll up to read history, auto-follow pauses
  // until they scroll back down.
  const capRef = useRef<HTMLDivElement>(null);
  const pinnedRef = useRef(true);

  useEffect(() => {
    const setupEventListeners = async () => {
      const unlistenShow = await listen("show-overlay", async (event) => {
        const overlayState = event.payload as OverlayState;
        // Reset synchronously before settings I/O. A fast microphone can emit
        // recording-ready while the awaits below are in flight; resetting after
        // them would overwrite that event and leave the overlay stuck arming.
        if (overlayState === "recording" || overlayState === "streaming") {
          setCaptureReady(false);
          smoothedLevelsRef.current = Array(16).fill(0);
          setLevels(Array(WAVE_BARS).fill(0));
          setStreamText({ committed: "", tentative: "" });
        }

        // The Live panel flows downward from a top overlay and upward from a
        // bottom one; read the placement so the layout can flip to match.
        try {
          const settings = await commands.getAppSettings();
          if (settings.status === "ok") {
            setPosition(
              settings.data.overlay_position === "top" ? "top" : "bottom",
            );
          }
        } catch {
          // Keep the previous/default placement if settings can't be read.
        }
        setState(overlayState);
        if (overlayState === "streaming") {
          setPhase("listening");
          setWorkKind("transcribing");
          setElapsed(0);
          setLoadNoticeShown(false);
          setSession((s) => s + 1); // remount the card fresh for this session
        }
        setIsVisible(true);
      });

      const unlistenNotice = await listen<OverlayNotice>(
        "overlay-notice",
        (event) => setNotice(event.payload),
      );

      const unlistenChanges = await listen<BarChanges>("bar-changes", (event) =>
        setChanges(event.payload),
      );

      const unlistenCall = await listen<CallCard>("bar-call", (event) =>
        setCallCard({ card: event.payload, at: Date.now() }),
      );

      const unlistenNotesTick = await events.notesTick.listen((event) =>
        setNotesTick(event.payload),
      );

      const unlistenPick = await listen<PickItem[]>("bar-pick", (event) => {
        setPicks(event.payload);
        setPickIndex(0);
      });
      const unlistenPickFocus = await listen<number>(
        "bar-pick-focus",
        (event) => setPickIndex(event.payload),
      );

      const unlistenLabel = await listen<string | null>(
        "overlay-working-label",
        (event) => setWorkingLabel(event.payload),
      );

      const unlistenHide = await listen("hide-overlay", () => {
        setIsVisible(false);
        setCaptureReady(false);
      });

      const unlistenReady = await listen("recording-ready", () => {
        setElapsed(0);
        setCaptureReady(true);
      });

      const unlistenLevel = await listen<number[]>("mic-level", (event) => {
        const newLevels = event.payload as number[];
        // Exponential smoothing across the 16 buckets, then take the first N
        // bars for the shared waveform.
        const smoothed = smoothedLevelsRef.current.map((prev, i) => {
          const target = newLevels[i] || 0;
          return prev * 0.7 + target * 0.3;
        });
        smoothedLevelsRef.current = smoothed;
        setLevels(smoothed.slice(0, WAVE_BARS));
      });

      const unlistenStream = await events.streamTextEvent.listen((event) => {
        setStreamText(event.payload);
      });

      const unlistenPhase = await events.streamPhaseEvent.listen((event) => {
        const payload: StreamPhaseEvent = event.payload;
        setPhase(payload.phase);
        if (payload.kind) setWorkKind(payload.kind);
      });

      // The backend ends every `loading_started` with exactly one of completed
      // or failed, so only those end a load. Other events (e.g. `unloaded` from
      // the idle watcher) aren't ordered against an in-flight load and must not
      // clear it.
      const unlistenModel = await listen<ModelStateEvent>(
        "model-state-changed",
        (event) => {
          const type = event.payload.event_type;
          if (type === "loading_started") {
            clearTimeout(modelLoadTimerRef.current);
            setModelLoading(true);
            setModelLoadSlow(false);
            modelLoadTimerRef.current = setTimeout(
              () => setModelLoadSlow(true),
              SLOW_MODEL_LOAD_MS,
            );
          } else if (
            type === "loading_completed" ||
            type === "loading_failed"
          ) {
            clearTimeout(modelLoadTimerRef.current);
            setModelLoading(false);
            setModelLoadSlow(false);
          }
        },
      );

      return () => {
        unlistenShow();
        unlistenNotice();
        unlistenChanges();
        unlistenPick();
        unlistenPickFocus();
        unlistenNotesTick();
        unlistenCall();
        unlistenLabel();
        unlistenHide();
        unlistenReady();
        unlistenLevel();
        unlistenStream();
        unlistenPhase();
        unlistenModel();
        clearTimeout(modelLoadTimerRef.current);
      };
    };

    setupEventListeners();
  }, []);

  // Elapsed capture timer starts only once microphone samples are flowing.
  useEffect(() => {
    if (state !== "streaming" || !isVisible || !captureReady) return;
    const id = setInterval(() => setElapsed((e) => e + 1), 1000);
    return () => clearInterval(id);
  }, [state, isVisible, captureReady]);

  // Stick to the bottom as text streams in — but only while pinned, so a user who
  // has scrolled up to read history isn't yanked back down by the next chunk.
  useLayoutEffect(() => {
    const el = capRef.current;
    if (!el) return;
    // Fade the top edge only once text actually overflows the cap.
    setOverflowing(el.scrollHeight > el.clientHeight + 1);
    if (pinnedRef.current) el.scrollTop = el.scrollHeight;
  }, [streamText]);

  // `session` re-runs this when a new Live session starts mid-load (e.g. a
  // retry during a cold load), since the reset doesn't change the other deps.
  useEffect(() => {
    if (modelLoadSlow && state === "streaming") setLoadNoticeShown(true);
  }, [modelLoadSlow, state, session]);

  // Each fresh streaming session starts pinned to the bottom, fade cleared.
  useEffect(() => {
    pinnedRef.current = true;
    setOverflowing(false);
  }, [session]);

  // The call card counts down, and its recorded time runs.
  useEffect(() => {
    if (state !== "call" || !isVisible) return;
    setNow(Date.now());
    const timer = setInterval(() => setNow(Date.now()), 250);
    return () => clearInterval(timer);
  }, [state, isVisible, callCard]);

  if (!isVisible) return null;

  // Re-pin when the user is within ~a line of the bottom; unpin otherwise.
  const handleStreamScroll = () => {
    const el = capRef.current;
    if (!el) return;
    pinnedRef.current = el.scrollHeight - el.scrollTop - el.clientHeight <= 16;
  };

  const fmtTime = (s: number) =>
    `${Math.floor(s / 60)}:${String(s % 60).padStart(2, "0")}`;

  const transcribingLabel = modelLoading
    ? t("overlay.loadingModel")
    : t("overlay.transcribing");

  // ---- Shared building blocks (one visual language for every overlay form) ----
  const waveform = (
    <div className={`swave ${captureReady ? "ready" : "arming"}`}>
      {levels.map((v, i) => (
        <i
          key={i}
          style={{
            height: `${Math.max(3, Math.min(18, 3 + Math.pow(v, 0.7) * 15))}px`,
          }}
        />
      ))}
    </div>
  );

  const cancelBtn = (
    <button
      className="sx"
      aria-label="cancel"
      onClick={() => commands.cancelOperation()}
    >
      <svg viewBox="0 0 16 16" aria-hidden="true">
        <path
          d="M4 4 L12 12 M12 4 L4 12"
          stroke="currentColor"
          strokeWidth="1.6"
          strokeLinecap="round"
        />
      </svg>
    </button>
  );

  // dot (left) | waveform (center) | timer + cancel (right) — same structure for
  // pill & panel, so the Live morph is a pure width change.
  const listeningRow = (showTimer: boolean, showCancel: boolean) => (
    <div className="sbase">
      <div className="sbase-l">
        <span className={`sdot ${captureReady ? "ready" : "arming"}`} />
      </div>
      {waveform}
      <div className="sbase-r">
        {showTimer && <span className="stimer">{fmtTime(elapsed)}</span>}
        {showCancel && cancelBtn}
      </div>
    </div>
  );

  // spinner (left) | label (center) | cancel (right) — same 3-zone grid as the
  // listening row, so the label is centered.
  const workingRow = (label: string, showCancel: boolean) => (
    <div className="sbase">
      <div className="sbase-l">
        <span className="sspinner" />
      </div>
      <span className="swork-label">{label}</span>
      <div className="sbase-r">{showCancel && cancelBtn}</div>
    </div>
  );

  // ---- Live overlay: a pill that sculpts open into a panel ----
  if (state === "streaming") {
    const hasText =
      streamText.committed.length > 0 || streamText.tentative.length > 0;
    const working = phase === "working";
    // While listening, a slow model load opens the panel with a notice where the
    // text would be, so speech that isn't transcribing yet doesn't look ignored.
    const loadingNotice = modelLoadSlow && !working && !hasText;
    // Keep the panel open whenever there's text — even while finalizing — so the
    // transcript stays put under a working spinner instead of collapsing and
    // squishing the text mid-stream. Only fall back to the small working pill
    // when there was no text to preserve.
    const open = hasText || loadingNotice || (loadNoticeShown && !working);
    const collapsed = working && !hasText;

    return (
      <div className={`ov-stage ${position}`}>
        <div
          key={session}
          className={`scard ${open ? "open" : ""} ${collapsed ? "working" : ""} ${
            isVisible ? "" : "leaving"
          }`}
        >
          <div className="stext">
            <div className="stext-clip">
              <div
                className={`stext-cap ${overflowing ? "overflowing" : ""}`}
                ref={capRef}
                onScroll={handleStreamScroll}
              >
                <p>
                  {loadingNotice && (
                    <span className="sloading">
                      {t("overlay.loadingModel")}
                    </span>
                  )}
                  <span className="committed">
                    {streamText.committed ? streamText.committed + " " : ""}
                  </span>
                  <span className="tentative">{streamText.tentative}</span>
                  {/* Drop the blinking caret once finalizing — it's no longer
                      capturing, and a static spinner conveys the work. */}
                  {!working && !loadingNotice && <span className="scaret" />}
                </p>
              </div>
            </div>
          </div>
          {working
            ? workingRow(
                workKind === "polishing"
                  ? t("overlay.processing")
                  : transcribingLabel,
                true,
              )
            : listeningRow(open, true)}
        </div>
      </div>
    );
  }

  const providerName = (id: string, label: string) =>
    id === "custom" ? t("ai.yourServer") : label;
  const problemText = (
    problem: AiProblemKind,
    providerId: string,
    label: string,
  ) =>
    t(
      providerId === "ollama" && problem === "unreachable"
        ? "overlay.notice.problem.unreachableOllama"
        : `overlay.notice.problem.${problem}`,
      { name: providerName(providerId, label) },
    );

  const noticeText = (n: OverlayNotice): string => {
    switch (n.kind) {
      case "word_added":
      case "word_exists":
        return t(`overlay.notice.${n.kind}`, { word: n.word });
      case "ai_skipped":
        return t(
          n.provider_id === "ollama" && n.problem === "unreachable"
            ? "overlay.notice.ai.unreachableOllama"
            : `overlay.notice.ai.${n.problem}`,
          { name: providerName(n.provider_id, n.provider_label) },
        );
      case "pasted":
        return n.app
          ? t("overlay.notice.pastedInto", { count: n.words, app: n.app })
          : t("overlay.notice.pasted", { count: n.words });
      case "transformed":
        return t("overlay.notice.transformed", {
          name: n.name,
          count: n.words,
        });
      case "copied":
        return t("overlay.notice.copied", { name: n.name });
      case "from_phone":
        return t(
          n.image
            ? "overlay.notice.fromPhoneImage"
            : "overlay.notice.fromPhone",
          { name: n.name },
        );
      case "pasted_last":
      case "copied_last":
        return t(`overlay.notice.${n.kind}`, { count: n.words });
      case "undone":
        return t(
          n.clipboard
            ? "overlay.notice.undoneClipboard"
            : "overlay.notice.undone",
        );
      case "spotted":
      case "learned":
        return t(
          `overlay.notice.${n.kind}${n.app ? "In" : ""}${n.more ? "More" : ""}`,
          { word: n.word, app: n.app, count: n.more },
        );
      case "saved_to_scratchpad":
        return n.admin && n.app
          ? t("overlay.notice.savedAdmin", { app: n.app })
          : n.app
            ? t("overlay.notice.savedFrom", { app: n.app })
            : t("overlay.notice.saved");
      case "note_ready":
        return t("overlay.notice.noteReady", {
          duration: durationLabel(n.duration_ms),
        });
      case "note_empty":
        return t("overlay.notice.noteEmpty");
      case "notes_failed":
        return t("overlay.notice.notesFailed", { reason: n.reason });
      case "call_never":
        return t("overlay.notice.callNever", { app: n.app });
      case "transform_failed":
        return t("overlay.notice.transformFailed", {
          name: n.name,
          problem: problemText(n.problem, n.provider_id, n.provider_label),
        });
      default:
        return t(`overlay.notice.${n.kind}`);
    }
  };

  // ---- Notice: one line of text that hides itself, with Undo after a paste.
  if (state === "notice") {
    const kind = notice?.kind ?? "";
    const canUndo =
      !!notice &&
      (notice.kind === "pasted" ||
        notice.kind === "transformed" ||
        notice.kind === "pasted_last" ||
        notice.kind === "learned") &&
      ("undo" in notice ? notice.undo : true);
    return (
      <div
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className="scard notice" role="status">
          {GOOD_NOTICES.has(kind) ? (
            <CheckMark />
          ) : WARN_NOTICES.has(kind) ? (
            <WarnMark />
          ) : (
            <span className="snotice-dot" />
          )}
          <span className="snotice-text">
            {notice ? noticeText(notice) : ""}
          </span>
          {canUndo && (
            <button
              type="button"
              className="sbtn ghost"
              onClick={() => commands.barUndo()}
            >
              {t("overlay.undo")}
            </button>
          )}
          {notice?.kind === "call_never" && (
            <button
              type="button"
              className="sbtn ghost"
              onClick={() => commands.callCardAction("undo_never")}
            >
              {t("overlay.undo")}
            </button>
          )}
          {notice?.kind === "saved_to_scratchpad" && (
            <button
              type="button"
              className="sbtn ghost"
              onClick={() =>
                commands.openScratchpad(notice.note_id).catch(() => {})
              }
            >
              {t("overlay.open")}
            </button>
          )}
          {notice?.kind === "note_ready" && (
            <button
              type="button"
              className="sbtn ghost"
              onClick={() => commands.openNote(notice.id)}
            >
              {t("overlay.open")}
            </button>
          )}
          {notice?.kind === "spotted" && (
            <button
              type="button"
              className="sbtn ghost"
              onClick={() => commands.barReview()}
            >
              {t("overlay.review")}
            </button>
          )}
        </div>
      </div>
    );
  }

  // ---- Call: notes offered for a call that started, or the call ended.
  if (state === "call" && callCard) {
    const { card, at } = callCard;
    const total = card.kind === "offer" ? card.countdown_ms : card.stop_in_ms;
    const left =
      total === null
        ? null
        : Math.max(0, Math.ceil((total - (now - at)) / 1000));
    const recorded =
      card.kind === "ended" ? clock(card.elapsed_ms + (now - at)) : "";
    const desc =
      card.kind === "offer"
        ? left !== null
          ? t("overlay.call.startsIn", { count: left })
          : card.test
            ? t("overlay.call.test")
            : t("overlay.call.offer", { app: card.name })
        : left !== null
          ? t("overlay.call.stopsIn", { count: left, recorded })
          : t("overlay.call.still", { recorded });
    return (
      <div
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div
          className="scard callcard"
          role="dialog"
          aria-label={
            card.kind === "offer"
              ? t("overlay.call.started", { app: card.name })
              : t("overlay.call.ended", { app: card.name })
          }
        >
          <div className="scall-hd">
            <CallIcon path={card.app_path} />
            <div className="scall-text">
              <div className="scall-title">
                {card.kind === "offer"
                  ? t("overlay.call.started", { app: card.name })
                  : t("overlay.call.ended", { app: card.name })}
              </div>
              <div className="scall-desc" aria-live="polite">
                {desc}
              </div>
            </div>
          </div>
          {total !== null && (
            <div className="scountdown" aria-hidden="true">
              <i
                key={at}
                style={{
                  animationDuration: `${total}ms`,
                }}
              />
            </div>
          )}
          <div className="spanel-actions">
            {card.kind === "offer" ? (
              <>
                <button
                  type="button"
                  className="sbtn primary"
                  onClick={() => commands.callCardAction("start")}
                >
                  {t("overlay.call.start")}
                </button>
                <button
                  type="button"
                  className="sbtn"
                  onClick={() => commands.callCardAction("not_now")}
                >
                  {t("overlay.call.notNow")}
                </button>
                <button
                  type="button"
                  className="sbtn quiet"
                  onClick={() => commands.callCardAction("never")}
                >
                  {t("overlay.call.never")}
                </button>
              </>
            ) : (
              <>
                <button
                  type="button"
                  className="sbtn primary"
                  onClick={() => commands.callCardAction("stop_now")}
                >
                  {left !== null
                    ? t("overlay.call.stopNow")
                    : t("overlay.call.stop")}
                </button>
                <button
                  type="button"
                  className="sbtn"
                  onClick={() => commands.callCardAction("keep")}
                >
                  {t("overlay.call.keep")}
                </button>
              </>
            )}
          </div>
        </div>
      </div>
    );
  }

  // ---- Notes: a recording runs; its clock, a meter per side, Pause and Stop.
  if (state === "notes") {
    const paused = notesTick?.paused ?? false;
    return (
      <div
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className="scard notes" role="status">
          <span className={`snotes-dot ${paused ? "paused" : ""}`} />
          <span className="snotes-label">{t("overlay.notes.label")}</span>
          <span className="snotes-clock">
            {clock(notesTick?.elapsed_ms ?? 0)}
          </span>
          <span className="snotes-meters" aria-hidden="true">
            <NotesMeter level={paused ? 0 : (notesTick?.you ?? 0)} />
            {notesTick?.them != null && (
              <NotesMeter level={paused ? 0 : notesTick.them} />
            )}
          </span>
          <button
            type="button"
            className="sx"
            aria-label={
              paused ? t("overlay.notes.resume") : t("overlay.notes.pause")
            }
            title={
              paused ? t("overlay.notes.resume") : t("overlay.notes.pause")
            }
            onClick={() => commands.pauseNotes(!paused)}
          >
            {paused ? (
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <path d="M5 3.5 L12.5 8 L5 12.5 Z" fill="currentColor" />
              </svg>
            ) : (
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <path
                  d="M5.5 4 V12 M10.5 4 V12"
                  stroke="currentColor"
                  strokeWidth="1.8"
                  strokeLinecap="round"
                />
              </svg>
            )}
          </button>
          <button
            type="button"
            className="sx"
            aria-label={t("overlay.notes.stop")}
            title={t("overlay.notes.stop")}
            onClick={() => commands.stopNotes()}
          >
            <svg viewBox="0 0 16 16" aria-hidden="true">
              <rect
                x="4"
                y="4"
                width="8"
                height="8"
                rx="1.5"
                fill="currentColor"
              />
            </svg>
          </button>
        </div>
      </div>
    );
  }

  // ---- Changes: the transform's word diff, with Keep and Undo.
  if (state === "changes" && changes) {
    const parts = wordDiff(changes.before, changes.after);
    return (
      <div
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div className="scard panel" role="dialog" aria-label={changes.name}>
          <div className="spanel-head">
            <span className="spanel-title">{changes.name}</span>
            {changes.keys && (
              <span className="spanel-keys">
                {t("overlay.changes.again", {
                  keys: formatKeyCombination(changes.keys),
                })}
              </span>
            )}
          </div>
          <div className="sdiff">
            {parts.map((part, i) =>
              part.kind === "same" ? (
                <span key={i}>{part.text}</span>
              ) : part.kind === "added" ? (
                <ins key={i}>{part.text}</ins>
              ) : (
                <del key={i}>{part.text}</del>
              ),
            )}
          </div>
          <div className="spanel-actions">
            <button
              type="button"
              className="sbtn primary"
              onClick={() => commands.barKeep()}
            >
              {t("overlay.changes.keep")}
            </button>
            <button
              type="button"
              className="sbtn"
              onClick={() => commands.barUndo()}
            >
              {t("overlay.undo")}
            </button>
          </div>
        </div>
      </div>
    );
  }

  // ---- Pick: which transform to run on the selection.
  if (state === "pick") {
    return (
      <div
        className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}
      >
        <div
          className="scard panel pick"
          role="dialog"
          aria-label={t("overlay.pick.title")}
        >
          <div className="spanel-head">
            <span className="spanel-title">{t("overlay.pick.title")}</span>
            <button
              type="button"
              className="sx"
              aria-label={t("overlay.pick.close")}
              onClick={() => commands.barDismiss()}
            >
              <svg viewBox="0 0 16 16" aria-hidden="true">
                <path
                  d="M4 4 L12 12 M12 4 L4 12"
                  stroke="currentColor"
                  strokeWidth="1.6"
                  strokeLinecap="round"
                />
              </svg>
            </button>
          </div>
          {picks.length === 0 ? (
            <p className="spick-empty">{t("overlay.pick.empty")}</p>
          ) : (
            <ul className="spick-list">
              {picks.map((item, i) => (
                <li key={item.id}>
                  <button
                    type="button"
                    className={`spick-row ${i === pickIndex ? "on" : ""}`}
                    aria-current={i === pickIndex ? "true" : undefined}
                    ref={(el) => {
                      if (i === pickIndex)
                        el?.scrollIntoView({ block: "nearest" });
                    }}
                    onClick={() => commands.barPick(item.id)}
                  >
                    <span className="spick-name">{item.name}</span>
                    {item.keys && (
                      <span className="spick-keys">
                        {formatKeyCombination(item.keys)}
                      </span>
                    )}
                  </button>
                </li>
              ))}
            </ul>
          )}
          {picks.length > 0 && (
            <p className="spick-hint">{t("overlay.pick.hint")}</p>
          )}
        </div>
      </div>
    );
  }

  // ---- Minimal overlay: exactly one row at a time — waveform (recording), or a
  // spinner + label (transcribing / processing). Never both. The pill animates its
  // width between them; the cancel button is in both rows so it stays put.
  const working = state === "transcribing" || state === "processing";
  const workLabel =
    state === "processing"
      ? workingLabel
        ? t("overlay.working", { name: workingLabel })
        : t("overlay.processing")
      : transcribingLabel;

  return (
    <div className={`ov-stage ${position} ov-fade ${isVisible ? "show" : ""}`}>
      <div
        className={`scard compact ${working && isVisible ? "cworking" : ""}`}
      >
        {working ? workingRow(workLabel, true) : listeningRow(false, true)}
      </div>
    </div>
  );
};

export default RecordingOverlay;
