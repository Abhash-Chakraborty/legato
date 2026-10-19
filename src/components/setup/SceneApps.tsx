import React, { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import {
  Check,
  CheckCheck,
  ChevronRight,
  Minus,
  Send,
  SendHorizontal,
  Square,
  X,
} from "lucide-react";
import type { ChatMessage } from "./setupStore";
import { blocks, type Scene } from "./setup";
import { AppGlyph, appOf } from "./parts";

/** A word in the text marked as fixed by the Dictionary, or as one Legato
 *  doesn't know yet. */
export interface Mark {
  text: string;
  kind: "fixed" | "unknown";
  tip: string;
}

/** Splits a line around its marks, each marked once, first come first. */
function marked(line: string, marks: Mark[]): React.ReactNode[] {
  const found: { at: number; mark: Mark }[] = [];
  for (const mark of marks) {
    const at = line.indexOf(mark.text);
    if (
      at >= 0 &&
      !found.some(
        (f) => at < f.at + f.mark.text.length && f.at < at + mark.text.length,
      )
    )
      found.push({ at, mark });
  }
  found.sort((a, b) => a.at - b.at);
  const out: React.ReactNode[] = [];
  let pos = 0;
  found.forEach(({ at, mark }, i) => {
    if (at > pos) out.push(line.slice(pos, at));
    out.push(
      <span
        key={i}
        title={mark.tip}
        className={
          mark.kind === "fixed"
            ? "underline decoration-teal-500 decoration-dotted decoration-2 underline-offset-[3px]"
            : "underline decoration-warn decoration-wavy underline-offset-[3px]"
        }
      >
        {mark.text}
      </span>,
    );
    pos = at + mark.text.length;
  });
  if (pos < line.length) out.push(line.slice(pos));
  return out;
}

/** Wraps each word so a new result can fade in word by word. */
function wordsIn(nodes: React.ReactNode[], counter: { n: number }) {
  return nodes.flatMap((node, k) => {
    if (typeof node !== "string")
      return [
        <span
          key={`m${k}`}
          className="setup-word"
          style={{ "--i": counter.n++ } as React.CSSProperties}
        >
          {node}
        </span>,
      ];
    return node.split(/(\s+)/).map((part, j) =>
      /^\s*$/.test(part) ? (
        part
      ) : (
        <span
          key={`${k}-${j}`}
          className="setup-word"
          style={{ "--i": counter.n++ } as React.CSSProperties}
        >
          {part}
        </span>
      ),
    );
  });
}

/** Text as the app shows it: paragraphs, lines and lists, with marks. */
export const SceneText: React.FC<{
  text: string;
  marks: Mark[];
  animate: boolean;
}> = ({ text, marks, animate }) => {
  const counter = { n: 0 };
  const line = (l: string) =>
    animate ? wordsIn(marked(l, marks), counter) : marked(l, marks);
  return (
    <div className={animate ? "setup-typing" : ""}>
      {blocks(text).map((b, i) =>
        b.kind === "p" ? (
          <p key={i} className="mb-3 last:mb-0">
            {b.lines.map((l, j) => (
              <React.Fragment key={j}>
                {j > 0 && <br />}
                {line(l)}
              </React.Fragment>
            ))}
          </p>
        ) : (
          React.createElement(
            b.kind,
            {
              key: i,
              className: `mb-3 ps-6 last:mb-0 ${b.kind === "ul" ? "list-disc" : "list-decimal"}`,
            },
            b.items.map((item, j) => (
              <li key={j} className="py-0.5">
                {line(item)}
              </li>
            )),
          )
        ),
      )}
    </div>
  );
};

/** Grey bars where the text is about to land. */
const Skeleton: React.FC<{ widths: number[] }> = ({ widths }) => (
  <div className="flex flex-col gap-2.5" aria-hidden="true">
    {widths.map((w, i) => (
      <span
        key={i}
        className="setup-skeleton block h-3 rounded-full bg-surface-3"
        style={{ width: `${w}%` }}
      />
    ))}
  </div>
);

const Caret = () => (
  <span
    className="setup-caret inline-block h-[1.1em] w-px translate-y-[0.2em] bg-text"
    aria-hidden="true"
  />
);

const Placeholder: React.FC<{ text: string }> = ({ text }) => (
  <p className="text-text-4">
    <Caret />
    {text}
  </p>
);

const WindowBar: React.FC<{ scene: Scene; title?: string }> = ({
  scene,
  title,
}) => (
  <div className="flex h-9 flex-none items-center gap-2 border-b border-line px-3 text-xs text-text-2">
    <AppGlyph scene={scene} />
    <span className="truncate">{title ?? appOf(scene)}</span>
    <span
      className="ms-auto flex items-center gap-3 text-text-4"
      aria-hidden="true"
    >
      <Minus className="h-3.5 w-3.5" />
      <Square className="h-3 w-3" />
      <X className="h-3.5 w-3.5" />
    </span>
  </div>
);

export interface SceneAppProps {
  scene: Scene;
  /** The text in the field, while it's there to send. */
  text: string | null;
  marks: Mark[];
  animate: boolean;
  phase: "idle" | "starting" | "recording" | "transcribing";
  chat: ChatMessage[];
  typing: boolean;
  emailSent: number | null;
  onSend: () => void;
}

const time = (at: number, locale: string) =>
  new Date(at).toLocaleTimeString(locale, {
    hour: "numeric",
    minute: "2-digit",
  });

/** A flat copy of the scene's app, holding whatever the dictation wrote. */
export const SceneApp: React.FC<SceneAppProps> = (p) => {
  const { t, i18n } = useTranslation();
  const chatRef = useRef<HTMLDivElement>(null);
  const opened = useRef(Date.now());
  useEffect(() => {
    chatRef.current?.scrollTo({ top: chatRef.current.scrollHeight });
  }, [p.chat.length, p.typing]);

  const listening = p.phase === "recording" || p.phase === "starting";
  const field =
    p.phase === "transcribing" ? (
      <Skeleton
        widths={
          p.scene === "message"
            ? [62]
            : p.scene === "email"
              ? [92, 100, 78, 40]
              : [54, 30, 36, 44]
        }
      />
    ) : p.text ? (
      <SceneText text={p.text} marks={p.marks} animate={p.animate} />
    ) : (
      <Placeholder text={t(`setup.scenes.${p.scene}.placeholder`)} />
    );
  const fieldRing = listening
    ? "shadow-[0_0_0_2px_var(--teal-300)]"
    : "shadow-none";

  if (p.scene === "message") {
    return (
      <div className="flex h-[440px] flex-col overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface shadow-[var(--shadow-1)]">
        <WindowBar scene="message" />
        <div className="flex flex-none items-center gap-3 border-b border-line px-4 py-2.5">
          <span
            className="flex h-8 w-8 items-center justify-center rounded-full bg-surface-3 text-xs font-medium text-text-2"
            aria-hidden="true"
          >
            {t("setup.scenes.message.initial")}
          </span>
          <div className="leading-tight">
            <b className="block text-sm font-medium">
              {t("setup.scenes.message.who")}
            </b>
            <span
              className={`text-xs ${p.typing ? "text-teal-700" : "text-text-3"}`}
            >
              {p.typing
                ? t("setup.scenes.message.typing")
                : t("setup.scenes.message.online")}
            </span>
          </div>
        </div>
        <div
          ref={chatRef}
          role="log"
          aria-label={t("setup.scenes.message.log")}
          className="flex min-h-0 flex-1 flex-col gap-1.5 overflow-y-auto bg-surface-2 px-4 py-3"
        >
          <span className="mx-auto mb-1 rounded-full bg-surface px-2.5 py-0.5 text-[11px] text-text-3">
            {t("setup.scenes.message.today")}
          </span>
          <Bubble
            dir="in"
            text={t("setup.scenes.message.asked")}
            at={time(opened.current - 4 * 60_000, i18n.language)}
          />
          {p.chat.map((m, i) => (
            <Bubble
              key={i}
              dir={m.dir}
              text={m.text}
              at={time(m.at, i18n.language)}
              read={t("setup.scenes.message.read")}
            />
          ))}
          {p.typing && (
            <div className="setup-bubble flex w-fit gap-1 rounded-[10px] bg-surface px-3 py-2.5">
              {[0, 1, 2].map((i) => (
                <i
                  key={i}
                  className="setup-dot block h-1.5 w-1.5 rounded-full bg-text-4"
                  style={{ animationDelay: `${i * 160}ms` }}
                />
              ))}
              <span className="sr-only">
                {t("setup.scenes.message.typing")}
              </span>
            </div>
          )}
        </div>
        <div className="flex flex-none items-end gap-2 border-t border-line px-3 py-2.5">
          <div
            role="textbox"
            aria-readonly="true"
            aria-label={t("setup.scenes.message.field")}
            className={`min-h-9 flex-1 rounded-[18px] bg-surface-2 px-4 py-2 text-sm transition-shadow duration-[var(--dur-2)] ${fieldRing}`}
          >
            {field}
          </div>
          <button
            type="button"
            onClick={p.onSend}
            disabled={!p.text || p.phase !== "idle"}
            aria-label={t("setup.scenes.send")}
            title={p.text ? undefined : t("setup.scenes.message.sendFirst")}
            className="flex h-9 w-9 flex-none items-center justify-center rounded-full bg-[#25d366] text-white transition-opacity disabled:bg-surface-3 disabled:text-text-4"
          >
            <SendHorizontal className="h-4 w-4" aria-hidden="true" />
          </button>
        </div>
      </div>
    );
  }

  if (p.scene === "email") {
    const sent = p.emailSent;
    return (
      <div className="flex h-[440px] flex-col overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface shadow-[var(--shadow-1)]">
        <WindowBar scene="email" title={t("setup.scenes.email.window")} />
        <div className="flex flex-none items-center gap-3 border-b border-line px-4 py-2">
          <button
            type="button"
            onClick={p.onSend}
            disabled={!p.text || !!sent || p.phase !== "idle"}
            className="flex h-7 items-center gap-1.5 rounded-[6px] bg-[#0f6cbd] px-3 text-[13px] font-medium text-white disabled:bg-surface-3 disabled:text-text-4"
          >
            {sent ? (
              <Check className="h-3.5 w-3.5" aria-hidden="true" />
            ) : (
              <Send className="h-3.5 w-3.5" aria-hidden="true" />
            )}
            {sent ? t("setup.scenes.email.sent") : t("setup.scenes.send")}
          </button>
          <span className="text-xs text-text-3">
            {sent
              ? t("setup.scenes.email.sentAt", {
                  time: time(sent, i18n.language),
                })
              : t("setup.scenes.email.draft")}
          </span>
        </div>
        <div className="flex-none border-b border-line px-4 text-[13px]">
          <div className="flex items-center gap-4 border-b border-line py-2">
            <span className="w-14 text-text-3">
              {t("setup.scenes.email.to")}
            </span>
            <span className="flex items-center gap-1.5 rounded-full bg-surface-2 py-0.5 ps-0.5 pe-2.5">
              <span
                className="flex h-5 w-5 items-center justify-center rounded-full bg-surface-3 text-[10px]"
                aria-hidden="true"
              >
                {t("setup.scenes.email.initial")}
              </span>
              {t("setup.scenes.email.who")}
            </span>
          </div>
          <div className="flex items-center gap-4 py-2">
            <span className="w-14 text-text-3">
              {t("setup.scenes.email.subjectLabel")}
            </span>
            <span>{t("setup.scenes.email.subject")}</span>
          </div>
        </div>
        <div
          role="textbox"
          aria-readonly="true"
          aria-label={t("setup.scenes.email.field")}
          className={`m-2 min-h-0 flex-1 overflow-y-auto rounded-[6px] px-3 py-2.5 text-sm leading-relaxed transition-shadow duration-[var(--dur-2)] ${fieldRing} ${sent ? "text-text-2" : ""}`}
        >
          {field}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-[440px] flex-col overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface shadow-[var(--shadow-1)]">
      <WindowBar scene="list" />
      <div className="flex flex-none items-center gap-1 px-4 py-2 text-xs text-text-3">
        <span>{t("setup.scenes.list.space")}</span>
        <ChevronRight className="h-3 w-3" aria-hidden="true" />
        <span>{t("setup.scenes.list.page")}</span>
      </div>
      <div className="min-h-0 flex-1 overflow-y-auto px-12 pt-6 pb-6">
        <h2 className="text-[26px] font-bold tracking-[-0.01em]">
          {t("setup.scenes.list.page")}
        </h2>
        <div
          role="textbox"
          aria-readonly="true"
          aria-label={t("setup.scenes.list.field")}
          className={`-mx-2 mt-3 rounded-[6px] px-2 py-1.5 text-[15px] leading-relaxed transition-shadow duration-[var(--dur-2)] ${fieldRing}`}
        >
          {field}
        </div>
      </div>
    </div>
  );
};

const Bubble: React.FC<{
  dir: "in" | "out";
  text: string;
  at: string;
  read?: string;
}> = ({ dir, text, at, read }) => (
  <div
    className={`setup-bubble max-w-[80%] rounded-[10px] px-3 py-1.5 text-sm shadow-[0_1px_0_rgba(0,0,0,0.06)] ${
      dir === "out" ? "setup-out self-end" : "self-start bg-surface"
    }`}
  >
    <span className="whitespace-pre-line">{text}</span>
    <span className="ms-2 inline-flex translate-y-0.5 items-center gap-0.5 text-[11px] opacity-60">
      {at}
      {dir === "out" && (
        <>
          <CheckCheck
            className="h-3.5 w-3.5 text-[#53bdeb]"
            aria-hidden="true"
          />
          <span className="sr-only">{read}</span>
        </>
      )}
    </span>
  </div>
);
