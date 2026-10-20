import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { Menu } from "@tauri-apps/api/menu";
import { Mic, NotebookPen, Square } from "lucide-react";
import {
  commands,
  events,
  type Edge,
  type Meeting,
  type SessionInfo,
} from "@/bindings";
import { formatKeyCombination } from "@/lib/utils/keyboard";
import "./Bubble.css";

interface Layout {
  expanded: boolean;
  dot_x: number;
  dot_y: number;
  edge: Edge;
}

type Action = "dictate" | "notes" | "scratchpad";

/** How far the pointer moves before a press becomes a drag. */
const DRAG_PX = 5;
/** Button centres either side of the dot, which sits on the middle one. */
const STEP = 46;
/** Bars in the level meter. */
const BARS = 5;

/** Which way the hovered button's label goes: towards the middle. */
const LABEL_SIDE: Record<Edge, string> = {
  left: "right",
  right: "left",
  top: "below",
  bottom: "above",
};

const clock = (ms: number) => {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, "0")}`;
};

/** A meeting starting this soon (seconds) shows in the Notes label. */
const SOON = 15 * 60;

/** The meeting going on now, or the next one starting soon. */
const meetingNow = (meetings: Meeting[], now: number) =>
  meetings
    .filter((m) => !m.all_day && m.start <= now && now < m.end)
    .sort((a, b) => b.start - a.start)[0] ??
  meetings.find((m) => !m.all_day && m.start > now && m.start - now <= SOON);

/** Legato's wave, in white, for the dot. */
const Wave: React.FC = () => (
  <svg viewBox="0 0 16 16" aria-hidden="true" className="bwave">
    {[
      [3, 6, 10],
      [6, 3.5, 12.5],
      [9, 5, 11],
      [12, 6.5, 9.5],
    ].map(([x, y1, y2]) => (
      <line
        key={x}
        x1={x + 0.5}
        x2={x + 0.5}
        y1={y1}
        y2={y2}
        stroke="currentColor"
        strokeWidth="1.8"
        strokeLinecap="round"
      />
    ))}
  </svg>
);

/** The microphone's level while you talk: bars lying across on a side of
 *  the screen, standing up at the top or bottom. While the words are being
 *  written up, the bars ripple instead. */
const Meter: React.FC<{ levels: number[]; upright: boolean }> = ({
  levels,
  upright,
}) => (
  <span className={`bmeter ${upright ? "v" : "h"}`} aria-hidden="true">
    {levels.map((l, i) => (
      <span
        key={i}
        style={
          {
            "--l": Math.min(1, Math.pow(l, 0.7)),
            "--i": i,
          } as React.CSSProperties
        }
      />
    ))}
  </span>
);

/**
 * The desktop bubble: a dot in the middle of a screen edge. Hovered, it
 * opens three buttons (Dictate, Notes, Scratchpad) with the hovered one's
 * label and shortcut towards the middle of the screen. While you talk it
 * becomes a level meter along its edge. Dragged, it moves; let go, it
 * settles in the middle of the nearest edge. Right-click hides it.
 */
const Bubble: React.FC = () => {
  const { t } = useTranslation();
  const [layout, setLayout] = useState<Layout>({
    expanded: false,
    dot_x: 20,
    dot_y: 44,
    edge: "right",
  });
  const [levels, setLevels] = useState<number[]>(() => Array(BARS).fill(0));
  const [hot, setHot] = useState<Action | null>(null);
  const [keys, setKeys] = useState<Record<Action, string>>({
    dictate: "",
    notes: "",
    scratchpad: "",
  });
  // A dictation: listening, or writing up what it heard. The bar leaves
  // both to the bubble while the bubble shows.
  const [phase, setPhase] = useState<"rec" | "work" | null>(null);
  const recording = phase === "rec";
  const [notes, setNotes] = useState<SessionInfo | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [panelOpen, setPanelOpen] = useState(false);
  const leaveTimer = useRef<number | null>(null);
  const press = useRef<{
    x: number;
    y: number;
    action: Action | null;
    dragging: boolean;
  } | null>(null);

  useEffect(() => {
    const subs = [
      listen<Layout>("bubble-layout", (e) => {
        setLayout(e.payload);
        if (!e.payload.expanded) setHot(null);
      }),
      // A dictation's states, sent here instead of the bar while the
      // bubble shows; the bar's other states reach every window too.
      listen<string>("show-overlay", (e) =>
        setPhase(
          e.payload === "recording" || e.payload === "streaming"
            ? "rec"
            : e.payload === "transcribing" || e.payload === "processing"
              ? "work"
              : null,
        ),
      ),
      listen("hide-overlay", () => {
        setPhase(null);
        setLevels(Array(BARS).fill(0));
      }),
      // The bar's sixteen bands, low to high, sent here only while the
      // bubble shows; speech sits in the lower ten.
      listen<number[]>("mic-level", (e) =>
        setLevels((prev) =>
          prev.map((p, i) => {
            const band = e.payload.slice(i * 2, i * 2 + 2);
            const v = band.length ? Math.max(...band) : 0;
            return p * 0.55 + v * 0.45;
          }),
        ),
      ),
      events.notesSessionChanged.listen((e) => setNotes(e.payload.session)),
      listen<boolean>("notes-panel", (e) => setPanelOpen(e.payload)),
      listen("bubble-dropped", () => {
        press.current = null;
      }),
      events.calendarChanged.listen(() =>
        commands
          .calendarMeetings()
          .then(setMeetings)
          .catch(() => {}),
      ),
    ];
    commands
      .calendarMeetings()
      .then(setMeetings)
      .catch(() => {});
    commands
      .getNotesSession()
      .then(setNotes)
      .catch(() => {});
    commands
      .getAppSettings()
      .then((r) => {
        if (r.status !== "ok") return;
        const b = r.data.bindings ?? {};
        setKeys({
          dictate: b.transcribe?.current_binding ?? "",
          notes: b.toggle_notes?.current_binding ?? "",
          scratchpad: b.open_scratchpad?.current_binding ?? "",
        });
      })
      .catch(() => {});
    return () => {
      subs.forEach((s) => s.then((off) => off()));
    };
  }, []);

  // The Notes clock runs while its label shows.
  useEffect(() => {
    if (!notes || notes.phase !== "recording") return;
    const started = Date.now() - notes.elapsed_ms;
    setElapsed(notes.elapsed_ms);
    const id = window.setInterval(
      () => setElapsed(notes.paused ? notes.elapsed_ms : Date.now() - started),
      1000,
    );
    return () => window.clearInterval(id);
  }, [notes]);

  const asked = useRef(0);
  const enter = () => {
    if (leaveTimer.current) window.clearTimeout(leaveTimer.current);
    leaveTimer.current = null;
    // Once per open: pointer moves would otherwise ask on every frame.
    if (layout.expanded || press.current?.dragging) return;
    if (Date.now() - asked.current < 400) return;
    asked.current = Date.now();
    commands.bubbleExpand(true);
  };
  const leave = () => {
    if (press.current?.dragging) return;
    if (leaveTimer.current) window.clearTimeout(leaveTimer.current);
    leaveTimer.current = window.setTimeout(() => {
      commands.bubbleExpand(false);
      setHot(null);
    }, 280);
  };

  const onPointerDown = (e: React.PointerEvent, action: Action | null) => {
    if (e.button !== 0) return;
    e.currentTarget.setPointerCapture(e.pointerId);
    press.current = {
      x: e.screenX,
      y: e.screenY,
      action,
      dragging: false,
    };
    // Legato watches the press too, and moves the bubble once it drags,
    // keeping the dot where the pointer took hold of it.
    commands.bubblePress(layout.dot_x - e.clientX, layout.dot_y - e.clientY);
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const p = press.current;
    if (!p || p.dragging) return;
    if (Math.hypot(e.screenX - p.x, e.screenY - p.y) < DRAG_PX) return;
    // A drag, which Legato is following: not a click.
    p.dragging = true;
    setHot(null);
  };

  const onPointerUp = () => {
    const p = press.current;
    press.current = null;
    if (p && !p.dragging && p.action) {
      commands.bubbleAction(p.action).catch(() => {});
    }
  };

  const onContextMenu = async (e: React.MouseEvent) => {
    e.preventDefault();
    const menu = await Menu.new({
      items: [
        {
          id: "snooze",
          text: t("bubble.menu.snooze"),
          action: () => commands.bubbleAction("snooze"),
        },
        {
          id: "off",
          text: t("bubble.menu.off"),
          action: () => commands.bubbleAction("off"),
        },
        { item: "Separator" },
        {
          id: "open",
          text: t("bubble.menu.open"),
          action: () => commands.bubbleAction("open"),
        },
      ],
    });
    await menu.popup();
  };

  const notesOn = !!notes && notes.phase === "recording";
  const now = Date.now() / 1000;
  const meeting = layout.expanded ? meetingNow(meetings, now) : undefined;
  const notesLabel = notesOn
    ? t("bubble.showNotes", { time: clock(elapsed) })
    : !meeting
      ? t("bubble.notes")
      : meeting.start <= now
        ? t("bubble.notesFor", { title: meeting.title })
        : t("bubble.notesSoon", {
            title: meeting.title,
            minutes: Math.max(1, Math.round((meeting.start - now) / 60)),
          });
  const upright = layout.edge === "left" || layout.edge === "right";
  const buttons: {
    id: Action;
    dy: number;
    label: string;
    on: boolean;
    icon: React.ReactNode;
  }[] = [
    {
      id: "dictate",
      dy: -STEP,
      label: recording ? t("bubble.stopDictating") : t("bubble.dictate"),
      on: recording,
      icon: recording ? (
        <Square aria-hidden="true" className="bstop" />
      ) : (
        <Mic aria-hidden="true" />
      ),
    },
    {
      id: "notes",
      dy: 0,
      label: notesLabel,
      on: notesOn,
      icon: <span className={`bnotes-mark ${notesOn ? "on" : ""}`} />,
    },
    {
      id: "scratchpad",
      dy: STEP,
      label: t("bubble.scratchpad"),
      on: false,
      icon: <NotebookPen aria-hidden="true" />,
    },
  ];

  return (
    <div
      className={`bstage ${layout.expanded ? "open" : ""}`}
      onPointerEnter={enter}
      // A dropped bubble moves out from under a still pointer, so the page
      // may not hear it come back; a move over the closed bubble opens it.
      onPointerMove={() => {
        if (!layout.expanded && !press.current) enter();
      }}
      onPointerLeave={leave}
      onContextMenu={onContextMenu}
    >
      {layout.expanded ? (
        buttons.map((b) => {
          const key = keys[b.id];
          return (
            <div
              key={b.id}
              className={`bitem labels-${LABEL_SIDE[layout.edge]}`}
              style={
                {
                  left: layout.dot_x + (upright ? 0 : b.dy),
                  top: layout.dot_y + (upright ? b.dy : 0),
                  // In a row, every label centres on the middle button.
                  "--shift": `${upright ? 0 : -b.dy}px`,
                } as React.CSSProperties
              }
            >
              <button
                type="button"
                className={`bbtn ${b.on ? "on" : ""} ${b.id}`}
                aria-label={b.label}
                onPointerEnter={() => setHot(b.id)}
                onPointerLeave={() => setHot((h) => (h === b.id ? null : h))}
                onPointerDown={(e) => onPointerDown(e, b.id)}
                onPointerMove={(e) => {
                  // The bubble opens under a still pointer, so the middle
                  // button may not hear it enter.
                  if (!press.current && hot !== b.id) setHot(b.id);
                  onPointerMove(e);
                }}
                onPointerUp={onPointerUp}
                onPointerCancel={onPointerUp}
              >
                {b.icon}
              </button>
              {hot === b.id && (
                <span className="blabel" role="tooltip">
                  <span className="btext">{b.label}</span>
                  {key && !b.on && (
                    <b className="bkeys">{formatKeyCombination(key)}</b>
                  )}
                </span>
              )}
            </div>
          );
        })
      ) : (
        <button
          type="button"
          className={`bdot ${phase ? `${phase} ${upright ? "tall" : "wide"}` : ""} ${notesOn && !panelOpen ? "notes" : ""}`}
          style={{ left: layout.dot_x, top: layout.dot_y }}
          aria-label={t("bubble.label")}
          onPointerDown={(e) => onPointerDown(e, null)}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
        >
          {phase ? <Meter levels={levels} upright={upright} /> : <Wave />}
        </button>
      )}
    </div>
  );
};

export default Bubble;
