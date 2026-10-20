import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { Menu } from "@tauri-apps/api/menu";
import { Mic, NotebookPen, Square } from "lucide-react";
import { commands, events, type SessionInfo } from "@/bindings";
import { formatKeyCombination } from "@/lib/utils/keyboard";
import "./Bubble.css";

interface Layout {
  expanded: boolean;
  dot_x: number;
  dot_y: number;
  labels: "left" | "right";
}

type Action = "dictate" | "notes" | "scratchpad";

/** How far the pointer moves before a press becomes a drag. */
const DRAG_PX = 5;
/** Button centres above and below the dot, which sits on the middle one. */
const STEP = 46;

const clock = (ms: number) => {
  const s = Math.floor(ms / 1000);
  const m = Math.floor(s / 60);
  return `${m}:${String(s % 60).padStart(2, "0")}`;
};

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

/**
 * The desktop bubble: a dot at the edge of the screen. Hovered, it opens
 * three buttons (Dictate, Notes, Scratchpad) with the hovered one's label
 * and shortcut beside it. Dragged, it moves; let go near a side, it snaps
 * there. Right-click hides it.
 */
const Bubble: React.FC = () => {
  const { t } = useTranslation();
  const [layout, setLayout] = useState<Layout>({
    expanded: false,
    dot_x: 20,
    dot_y: 20,
    labels: "left",
  });
  const [hot, setHot] = useState<Action | null>(null);
  const [keys, setKeys] = useState<Record<Action, string>>({
    dictate: "",
    notes: "",
    scratchpad: "",
  });
  const [recording, setRecording] = useState(false);
  const [notes, setNotes] = useState<SessionInfo | null>(null);
  const [elapsed, setElapsed] = useState(0);
  const leaveTimer = useRef<number | null>(null);
  const press = useRef<{
    x: number;
    y: number;
    grabX: number;
    grabY: number;
    action: Action | null;
    dragging: boolean;
  } | null>(null);
  const frame = useRef(0);

  useEffect(() => {
    const subs = [
      listen<Layout>("bubble-layout", (e) => {
        setLayout(e.payload);
        if (!e.payload.expanded) setHot(null);
      }),
      // The bar's states, which every window hears: the mic button shows
      // a dictation running.
      listen<string>("show-overlay", (e) =>
        setRecording(e.payload === "recording" || e.payload === "streaming"),
      ),
      listen("hide-overlay", () => setRecording(false)),
      events.notesSessionChanged.listen((e) => setNotes(e.payload.session)),
    ];
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

  const enter = () => {
    if (leaveTimer.current) window.clearTimeout(leaveTimer.current);
    leaveTimer.current = null;
    if (!layout.expanded && !press.current?.dragging) {
      commands.bubbleExpand(true);
    }
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
      // Where in the dot the pointer took hold, so the dot doesn't jump.
      grabX: e.clientX - layout.dot_x,
      grabY: e.clientY - layout.dot_y,
      action,
      dragging: false,
    };
  };

  const onPointerMove = (e: React.PointerEvent) => {
    const p = press.current;
    if (!p) return;
    if (!p.dragging && Math.hypot(e.screenX - p.x, e.screenY - p.y) < DRAG_PX) {
      return;
    }
    p.dragging = true;
    setHot(null);
    const x = e.screenX - p.grabX;
    const y = e.screenY - p.grabY;
    cancelAnimationFrame(frame.current);
    frame.current = requestAnimationFrame(() => commands.bubbleMoveTo(x, y));
  };

  const onPointerUp = () => {
    const p = press.current;
    press.current = null;
    if (!p) return;
    if (p.dragging) {
      cancelAnimationFrame(frame.current);
      commands.bubbleDrop();
      return;
    }
    if (p.action) commands.bubbleAction(p.action).catch(() => {});
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
      label: notesOn
        ? t("bubble.stopNotes", { time: clock(elapsed) })
        : t("bubble.notes"),
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
      onPointerLeave={leave}
      onContextMenu={onContextMenu}
    >
      {layout.expanded ? (
        buttons.map((b) => {
          const key = keys[b.id];
          return (
            <div
              key={b.id}
              className={`bitem labels-${layout.labels}`}
              style={{ left: layout.dot_x, top: layout.dot_y + b.dy }}
            >
              <button
                type="button"
                className={`bbtn ${b.on ? "on" : ""} ${b.id}`}
                aria-label={b.label}
                onPointerEnter={() => setHot(b.id)}
                onPointerLeave={() => setHot((h) => (h === b.id ? null : h))}
                onPointerDown={(e) => onPointerDown(e, b.id)}
                onPointerMove={onPointerMove}
                onPointerUp={onPointerUp}
                onPointerCancel={onPointerUp}
              >
                {b.icon}
              </button>
              {hot === b.id && (
                <span className="blabel" role="tooltip">
                  {b.label}
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
          className={`bdot ${recording ? "rec" : ""} ${notesOn ? "notes" : ""}`}
          style={{ left: layout.dot_x, top: layout.dot_y }}
          aria-label={t("bubble.label")}
          onPointerDown={(e) => onPointerDown(e, null)}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={onPointerUp}
        >
          <Wave />
        </button>
      )}
    </div>
  );
};

export default Bubble;
