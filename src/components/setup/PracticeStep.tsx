import React, {
  useCallback,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import {
  BookA,
  Check,
  CircleHelp,
  CircleSlash,
  Eraser,
  Keyboard,
  List,
  ListOrdered,
  Loader2,
  Mic,
  MicOff,
  Pointer,
  Square,
  TriangleAlert,
  Type,
  Undo2,
  type LucideIcon,
} from "lucide-react";
import { commands, events, type Tone } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import { useSettingsStore } from "../../stores/settingsStore";
import { tonesFor } from "../pages/style/model";
import { ActionMenu } from "../ui/ActionMenu";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import type { StepProps } from "./SetupFlow";
import { Keys, Panel, Row, appOf } from "./parts";
import { SceneApp, type Mark } from "./SceneApps";
import {
  PRACTICE,
  SCENE_CATEGORY,
  SCENE_TERMS,
  countWords,
  droppedRuns,
  listOf,
  practiceMode,
  struckRuns,
  termAsHeard,
  type Scene,
} from "./setup";
import { sceneDone, useSetupStore } from "./setupStore";

type Phase = "idle" | "starting" | "recording" | "transcribing";
type Msg =
  | "idle"
  | "again"
  | "rec"
  | "lock"
  | "key"
  | "trans"
  | "short"
  | "cancel"
  | "sent"
  | "done"
  | "tone";

const MSG_ICON: Record<Msg, LucideIcon | "live" | "spin"> = {
  idle: Mic,
  again: Mic,
  rec: "live",
  lock: "live",
  key: "live",
  trans: "spin",
  short: TriangleAlert,
  cancel: CircleSlash,
  sent: Check,
  done: Check,
  tone: Check,
};

/** The script with the parts the rules will change underlined. */
const Script: React.FC<{ text: string; marks: string[] }> = ({
  text,
  marks,
}) => {
  const parts: React.ReactNode[] = [];
  let rest = text;
  let k = 0;
  for (const m of marks) {
    const at = rest.indexOf(m);
    if (at < 0) continue;
    parts.push(rest.slice(0, at));
    parts.push(
      <u
        key={k++}
        className="decoration-teal-500 decoration-2 underline-offset-[4px]"
      >
        {m}
      </u>,
    );
    rest = rest.slice(at + m.length);
  }
  parts.push(rest);
  return <>{parts}</>;
};

/** One practice scene: read the line with the mic button or the shortcut,
 *  and the text lands in a copy of the app, as Legato would paste it. */
export const PracticeStep: React.FC<StepProps & { scene: Scene }> = ({
  scene,
  go,
  setFoot,
}) => {
  const { t, i18n } = useTranslation();
  const { settings } = useSettings();
  const refreshSettings = useSettingsStore((s) => s.refreshSettings);
  const result = useSetupStore((s) => s.results[scene]) ?? null;
  const results = useSetupStore((s) => s.results);
  const chat = useSetupStore((s) => s.chat);
  const emailSent = useSetupStore((s) => s.emailSent);
  const setResult = useSetupStore((s) => s.setResult);
  const send = useSetupStore((s) => s.send);
  const receive = useSetupStore((s) => s.receive);

  const [phase, setPhaseState] = useState<Phase>("idle");
  const [msg, setMsg] = useState<{ kind: Msg; extra?: string }>(() => ({
    kind: result ? "again" : "idle",
  }));
  const [locked, setLocked] = useState(false);
  const [animate, setAnimate] = useState(false);
  const [said, setSaid] = useState(false);
  const [seconds, setSeconds] = useState(0);
  const [typing, setTyping] = useState(false);
  const [tonesDiffer, setTonesDiffer] = useState(false);
  const [menu, setMenu] = useState(false);
  const [savedTick, setSavedTick] = useState(false);
  const [stuck, setStuck] = useState(false);
  const [help, setHelp] = useState(false);
  const [ping, setPing] = useState(false);

  const phaseRef = useRef<Phase>("idle");
  const src = useRef<"button" | "key">("key");
  const t0 = useRef(0);
  const lastPress = useRef(0);
  const gotResult = useRef(false);
  const holdRef = useRef<HTMLButtonElement>(null);
  const timers = useRef<number[]>([]);

  const setPhase = (p: Phase) => {
    phaseRef.current = p;
    setPhaseState(p);
  };
  const later = (fn: () => void, ms: number) => {
    timers.current.push(window.setTimeout(fn, ms));
  };

  const activation = settings?.shortcut_activation ?? "hold_or_toggle";
  const toggleMode = activation === "toggle";
  const threshold = settings?.hold_threshold_ms ?? 300;
  const binding = settings?.bindings?.transcribe?.current_binding ?? "";
  const category = SCENE_CATEGORY[scene];
  const app = appOf(scene);
  const script = t(`setup.scenes.${scene}.script`);
  const done = sceneDone({ results, chat }, scene);

  useEffect(
    () => () => {
      timers.current.forEach(clearTimeout);
      // Leaving mid-dictation drops it rather than pasting it somewhere.
      if (phaseRef.current !== "idle") commands.cancelOperation();
    },
    [],
  );

  // After 8 s without a dictation, offer help.
  useEffect(() => {
    if (done || phase !== "idle") return;
    const id = window.setTimeout(() => setStuck(true), 8000);
    return () => clearTimeout(id);
  }, [done, phase]);

  // The dictation's progress, from the shortcut or the button alike.
  useEffect(() => {
    const offStatus = events.practiceStatus.listen(({ payload }) => {
      const prev = phaseRef.current;
      if (payload.phase === "recording") {
        if (prev === "idle") {
          src.current = "key";
          setLocked(false);
        }
        t0.current = prev === "starting" ? t0.current : Date.now();
        setPhase("recording");
        setSaid(false);
        gotResult.current = false;
        setMsg({
          kind:
            src.current === "key"
              ? "key"
              : locked || toggleMode
                ? "lock"
                : "rec",
        });
      } else if (payload.phase === "transcribing") {
        setPhase("transcribing");
        setMsg({ kind: "trans" });
      } else if (payload.phase === "cancelled") {
        setPhase("idle");
        setLocked(false);
        setMsg({ kind: "cancel" });
      } else if (prev !== "idle") {
        setPhase("idle");
        setLocked(false);
        if (!gotResult.current) setMsg({ kind: "short" });
      }
    });
    const offResult = events.practiceResult.listen(({ payload }) => {
      if (payload.scene !== practiceMode(scene)) return;
      gotResult.current = true;
      setAnimate(true);
      setResult(scene, payload);
      setStuck(false);
      setMsg({
        kind: "done",
        extra: t("setup.practice.landed", {
          count: countWords(payload.text),
          seconds: (payload.ms / 1000).toLocaleString(i18n.language, {
            maximumFractionDigits: 1,
            minimumFractionDigits: 1,
          }),
        }),
      });
    });
    return () => {
      offStatus.then((fn) => fn());
      offResult.then((fn) => fn());
    };
  }, [scene, locked, toggleMode, setResult, t, i18n.language]);

  // The recording clock.
  useEffect(() => {
    if (phase !== "recording") return;
    setSeconds(0);
    const id = window.setInterval(
      () => setSeconds(Math.floor((Date.now() - t0.current) / 1000)),
      250,
    );
    return () => clearInterval(id);
  }, [phase]);

  // The style line shows only where the tones give different text: in a
  // document every tone writes the same list.
  useEffect(() => {
    if (!result || result.tone === null) {
      setTonesDiffer(false);
      return;
    }
    let live = true;
    Promise.all(
      tonesFor(category).map((tone) =>
        commands.previewStyle(result.heard, category, tone, null),
      ),
    )
      .then((texts) => live && setTonesDiffer(new Set(texts).size > 1))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [result, category]);

  // ---- the mic button: hold to talk, or tap to keep going until the next
  // press, following Shortcut behaviour.
  const press = useCallback(() => {
    const p = phaseRef.current;
    if (p === "idle") {
      src.current = "button";
      t0.current = Date.now();
      setLocked(toggleMode);
      setPhase("starting");
      setMsg({ kind: toggleMode ? "lock" : "rec" });
      commands.practiceToggle();
    } else if (
      (p === "recording" || p === "starting") &&
      (locked || src.current === "key")
    ) {
      commands.practiceToggle();
    }
  }, [toggleMode, locked]);

  const release = useCallback(() => {
    const p = phaseRef.current;
    if (src.current !== "button" || locked) return;
    if (p !== "recording" && p !== "starting") return;
    if (toggleMode) return;
    if (
      activation === "hold_or_toggle" &&
      Date.now() - t0.current < threshold
    ) {
      setLocked(true);
      setMsg({ kind: "lock" });
      return;
    }
    commands.practiceToggle();
  }, [locked, toggleMode, activation, threshold]);

  // ---- sending, in the chat and the email
  const onSend = () => {
    if (!result || phase !== "idle") return;
    const first = scene === "message" && !chat.some((m) => m.dir === "out");
    send(scene);
    setMsg({ kind: "sent" });
    setAnimate(false);
    if (first) {
      later(() => setTyping(true), 650);
      later(() => {
        setTyping(false);
        receive({
          dir: "in",
          text: t("setup.scenes.message.reply"),
          at: Date.now(),
        });
      }, 2100);
    }
  };

  const changeTone = async (tone: Tone) => {
    if (!result || !settings?.styles || phase !== "idle") return;
    const style = settings.styles[category];
    const saved = await commands.changeCategoryStyleSetting(category, {
      ...style,
      tone,
    });
    if (saved.status !== "ok") return;
    refreshSettings();
    const text = await commands.previewStyle(
      result.heard,
      category,
      tone,
      null,
    );
    setAnimate(false);
    setResult(scene, { ...result, text, tone });
    setMsg({
      kind: "tone",
      extra: t("setup.did.toneApplied", {
        tone: t(`style.tones.${tone}.label`),
      }),
    });
    setSavedTick(true);
    later(() => setSavedTick(false), 1400);
  };

  // ---- the footer
  useEffect(() => {
    const next =
      scene === "list" ? "words" : PRACTICE[PRACTICE.indexOf(scene) + 1];
    setFoot(
      done
        ? { onNext: () => go(next) }
        : scene === "message"
          ? { disabled: true, note: t("setup.practice.firstNote") }
          : {
              disabled: true,
              note: (
                <Button variant="ghost" size="sm" onClick={() => go(next)}>
                  {t("setup.practice.skipOne")}
                </Button>
              ),
            },
    );
  }, [done, scene, go, setFoot, t]);

  // ---- what lands, and what Legato did to it
  const unknown = useMemo(
    () =>
      result
        ? SCENE_TERMS[scene]
            .map((term) => ({
              term,
              heard: termAsHeard(term, script, result.text),
            }))
            .filter(
              (u): u is { term: string; heard: string } =>
                !!u.heard && u.heard.toLowerCase() !== u.term.toLowerCase(),
            )
        : [],
    [result, scene, script],
  );
  const marks: Mark[] = result
    ? [
        ...result.fixes.map((f) => ({
          text: f.to,
          kind: "fixed" as const,
          tip: t("setup.did.fixedTip", { word: f.to, from: f.from }),
        })),
        ...unknown.map((u) => ({
          text: u.heard,
          kind: "unknown" as const,
          tip: t("setup.did.unknownTip", { word: u.term }),
        })),
      ]
    : [];

  const quote = (s: string) => t("setup.quote", { text: s });
  const join = (list: string[]) =>
    list.length < 2
      ? list.join("")
      : t("setup.and", {
          list: list.slice(0, -1).join(", "),
          last: list[list.length - 1],
        });

  const rows: { icon: LucideIcon; text: React.ReactNode; warn?: boolean }[] =
    [];
  if (result) {
    if (result.fillers.length)
      rows.push({
        icon: Eraser,
        text: t("setup.did.fillers", { list: join(result.fillers.map(quote)) }),
      });
    for (const run of droppedRuns(result.uncorrected, result.corrected))
      rows.push({ icon: Undo2, text: t("setup.did.corrected", { run }) });
    for (const f of result.fixes)
      rows.push({
        icon: BookA,
        text: t(f.source === "ai" ? "setup.did.fixedAi" : "setup.did.fixed", {
          word: f.to,
          from: f.from,
        }),
      });
    for (const u of unknown)
      rows.push({
        icon: CircleHelp,
        warn: true,
        text: t("setup.did.unknown", { heard: u.heard, word: u.term }),
      });
    const list = listOf(result.text);
    if (list)
      rows.push({
        icon: list.numbered ? ListOrdered : List,
        text: t("setup.did.list", { count: list.items, app }),
      });
    if (result.tone && tonesDiffer)
      rows.push({
        icon: Type,
        text: (
          <>
            {t("setup.did.tone", {
              tone: t(`style.tones.${result.tone}.label`),
              kind: t(`style.categories.${category}`).toLowerCase(),
              help: t(`style.tones.${result.tone}.help`).toLowerCase(),
            })}{" "}
            <span className="relative inline-block">
              <button
                type="button"
                className="link"
                aria-haspopup="menu"
                aria-expanded={menu}
                onClick={() => setMenu((m) => !m)}
              >
                {t("setup.did.change")}
              </button>
              {menu && (
                <ActionMenu
                  label={t("setup.did.changeTone")}
                  className="start-0 top-full mt-1"
                  onClose={(restore) => {
                    setMenu(false);
                    if (restore) holdRef.current?.focus();
                  }}
                  items={tonesFor(category).map((tone) => ({
                    id: tone,
                    label: t(`style.tones.${tone}.label`),
                    checked: tone === result.tone,
                    onSelect: () => {
                      setMenu(false);
                      if (tone !== result.tone) changeTone(tone);
                    },
                  }))}
                />
              )}
            </span>
            <span
              className={`saved ms-2 ${savedTick ? "show" : ""}`}
              aria-live="polite"
            >
              {savedTick && t("setup.did.saved")}
            </span>
          </>
        ),
      });
    else if (result.tone === null)
      rows.push({ icon: Type, text: t("setup.did.styleOff", { app }) });
    if (rows.length === 0)
      rows.push({ icon: Check, text: t("setup.did.nothing") });
  }

  // The chat's field empties once the reply is sent.
  const fieldText = result?.text ?? null;
  const busy = phase !== "idle";
  const holdLabel = busy
    ? locked || toggleMode || src.current === "key"
      ? t("setup.practice.pressStop")
      : t("setup.practice.release")
    : toggleMode
      ? t("setup.practice.pressTalk")
      : t("setup.practice.holdTalk");

  const msgText = (() => {
    switch (msg.kind) {
      case "idle":
        return toggleMode
          ? t("setup.practice.msg.idleToggle", { app })
          : t("setup.practice.msg.idle", { app });
      case "again":
        return toggleMode
          ? t("setup.practice.msg.againToggle")
          : t("setup.practice.msg.again");
      case "short":
        return toggleMode
          ? t("setup.practice.msg.shortToggle")
          : t("setup.practice.msg.short");
      case "done":
      case "tone":
        return msg.extra ?? "";
      case "sent":
        return t(`setup.practice.msg.sent.${scene}`);
      default:
        return t(`setup.practice.msg.${msg.kind}`);
    }
  })();
  const icon = MSG_ICON[msg.kind];
  const MsgIcon = typeof icon === "string" ? null : icon;
  const clock = `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, "0")}`;

  return (
    <Panel
      title={t(`setup.scenes.${scene}.title`)}
      sub={t(`setup.scenes.${scene}.sub`)}
      left={
        <>
          <div
            className={`mt-6 rounded-[var(--r-lg)] p-4 ${
              said ? "border border-dashed border-line-strong" : "bg-surface-2"
            }`}
          >
            <div className="text-xs text-text-3">
              {said ? t("setup.practice.heardLabel") : t("setup.practice.say")}
            </div>
            <p className="mt-1.5 text-[15px] leading-relaxed">
              {said && result ? (
                struckRuns(result.heard, result.text).map((r, i) =>
                  r.removed ? (
                    <React.Fragment key={i}>
                      <s className="text-text-3">{r.text}</s>
                      <span className="sr-only">
                        {t("setup.practice.removed")}
                      </span>{" "}
                    </React.Fragment>
                  ) : (
                    <React.Fragment key={i}>{r.text} </React.Fragment>
                  ),
                )
              ) : (
                <>
                  {"“"}
                  <Script
                    text={script}
                    marks={
                      t(`setup.scenes.${scene}.marks`, {
                        returnObjects: true,
                      }) as string[]
                    }
                  />
                  {"”"}
                </>
              )}
            </p>
          </div>

          <div className="mt-5 flex items-center gap-4">
            <button
              ref={holdRef}
              type="button"
              aria-pressed={busy}
              aria-label={holdLabel}
              onPointerDown={(e) => {
                if (e.button > 0) return;
                e.preventDefault();
                lastPress.current = Date.now();
                try {
                  holdRef.current?.setPointerCapture(e.pointerId);
                } catch {
                  // Capture is optional.
                }
                press();
              }}
              onPointerUp={() => {
                lastPress.current = Date.now();
                release();
              }}
              onPointerCancel={release}
              onKeyDown={(e) => {
                if ((e.key === " " || e.key === "Enter") && !e.repeat) {
                  e.preventDefault();
                  e.stopPropagation();
                  lastPress.current = Date.now();
                  press();
                }
              }}
              onKeyUp={(e) => {
                if (e.key === " " || e.key === "Enter") {
                  e.preventDefault();
                  release();
                }
              }}
              onClick={(e) => {
                // A bare click (screen readers) toggles.
                if (e.detail !== 0 || Date.now() - lastPress.current < 700)
                  return;
                if (phaseRef.current === "idle") {
                  setLocked(true);
                  press();
                } else commands.practiceToggle();
              }}
              onContextMenu={(e) => e.preventDefault()}
              onAnimationEnd={() => setPing(false)}
              className={`setup-mic relative flex h-16 w-16 flex-none touch-none items-center justify-center rounded-full border-2 transition-colors duration-[var(--dur-2)] ${
                busy
                  ? "border-teal-600 bg-teal-600 text-white"
                  : "border-text bg-surface text-text hover:bg-surface-2"
              } ${ping ? "setup-ping" : ""}`}
            >
              {busy && phase !== "transcribing" && (
                <span className="setup-ring" aria-hidden="true" />
              )}
              {phase === "transcribing" ? (
                <Loader2 className="h-6 w-6 animate-spin" aria-hidden="true" />
              ) : busy && (locked || toggleMode) ? (
                <Square className="h-5 w-5" aria-hidden="true" />
              ) : (
                <Mic className="h-6 w-6" aria-hidden="true" />
              )}
            </button>
            <div className="min-w-0">
              <b className="block text-sm font-medium">{holdLabel}</b>
              {binding && (
                <span className="mt-1 flex flex-wrap items-center gap-1.5 text-[13px] text-text-3">
                  {toggleMode
                    ? t("setup.practice.orPress")
                    : t("setup.practice.orHold")}
                  <Keys binding={binding} />
                </span>
              )}
            </div>
          </div>

          <div
            className={`mt-4 flex items-center gap-2 text-[13px] ${
              msg.kind === "short"
                ? "text-warn"
                : ["done", "sent", "tone"].includes(msg.kind)
                  ? "text-text"
                  : "text-text-2"
            }`}
          >
            <span
              className="flex h-4 w-4 flex-none items-center justify-center"
              aria-hidden="true"
            >
              {icon === "live" ? (
                <span className="setup-live h-2 w-2 rounded-full bg-danger" />
              ) : icon === "spin" ? (
                <Loader2 className="h-3.5 w-3.5 animate-spin" />
              ) : MsgIcon ? (
                <MsgIcon
                  className={`h-3.5 w-3.5 ${["done", "sent", "tone"].includes(msg.kind) ? "text-teal-600" : ""}`}
                />
              ) : null}
            </span>
            <span aria-live="polite">{msgText}</span>
            {phase === "recording" && (
              <span
                className="ms-auto tabular-nums text-text-3"
                aria-hidden="true"
              >
                {clock}
              </span>
            )}
          </div>

          {result && !busy && (
            <div className="mt-5 rounded-[var(--r-lg)] border border-line">
              <div className="flex items-center justify-between gap-3 border-b border-line px-4 py-2.5">
                <b className="text-[13px] font-medium">
                  {t("setup.did.title")}
                </b>
                <button
                  type="button"
                  className="link text-[13px]"
                  aria-pressed={said}
                  onClick={() => setSaid((s) => !s)}
                >
                  {said ? t("setup.did.showLine") : t("setup.did.showSaid")}
                </button>
              </div>
              <ul className="flex flex-col gap-2.5 px-4 py-3">
                {rows.map((row, i) => (
                  <li
                    key={i}
                    className={`setup-row flex gap-2.5 text-[13px] leading-snug ${row.warn ? "text-warn" : "text-text-2"}`}
                    style={{ animationDelay: animate ? `${i * 40}ms` : "0ms" }}
                  >
                    <row.icon
                      className="mt-px h-3.5 w-3.5 flex-none"
                      aria-hidden="true"
                    />
                    <span>{row.text}</span>
                  </li>
                ))}
              </ul>
            </div>
          )}

          {stuck && !done && !busy && (
            <button
              type="button"
              className="link mt-4 text-[13px]"
              onClick={() => setHelp(true)}
            >
              {t("setup.help.open")}
            </button>
          )}

          <Dialog
            open={help}
            onOpenChange={setHelp}
            title={t("setup.help.title")}
            closeLabel={t("common.close")}
          >
            <div className="overflow-hidden rounded-[var(--r-lg)] border border-line">
              <Row
                icon={<MicOff className="h-4 w-4" />}
                title={t("setup.help.mic")}
                help={t("setup.help.micHelp")}
                aside={
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => {
                      setHelp(false);
                      go("mic");
                    }}
                  >
                    {t("setup.help.check")}
                  </Button>
                }
              />
              <Row
                icon={<Keyboard className="h-4 w-4" />}
                title={t("setup.help.key")}
                help={t("setup.help.keyHelp")}
              />
              <Row
                icon={<Pointer className="h-4 w-4" />}
                title={t("setup.help.button")}
                help={
                  toggleMode
                    ? t("setup.help.buttonHelpToggle")
                    : t("setup.help.buttonHelp")
                }
                aside={
                  <Button
                    variant="secondary"
                    size="sm"
                    onClick={() => {
                      setHelp(false);
                      later(() => {
                        holdRef.current?.focus({ preventScroll: true });
                        setPing(true);
                      }, 220);
                    }}
                  >
                    {t("setup.help.show")}
                  </Button>
                }
              />
            </div>
          </Dialog>
        </>
      }
      right={
        <SceneApp
          scene={scene}
          text={fieldText}
          marks={marks}
          animate={animate}
          phase={phase}
          chat={chat}
          typing={typing}
          emailSent={emailSent}
          onSend={onSend}
        />
      }
    />
  );
};
