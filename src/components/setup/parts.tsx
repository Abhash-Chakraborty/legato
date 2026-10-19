import React from "react";
import { FileText, Mail, MessageCircle } from "lucide-react";
import { formatKeyCombination } from "../../lib/utils/keyboard";
import type { Scene } from "./setup";

/** A setup step: title and explanation on the left, the thing to look at on
 *  the right. */
export const Panel: React.FC<{
  title: string;
  sub?: React.ReactNode;
  left?: React.ReactNode;
  right: React.ReactNode;
}> = ({ title, sub, left, right }) => (
  <div className="grid grid-cols-1 gap-10 min-[880px]:grid-cols-[minmax(0,330px)_minmax(0,1fr)] min-[880px]:gap-12">
    <div className="min-w-0">
      <h1
        tabIndex={-1}
        className="text-[22px] font-semibold tracking-[-0.01em] focus:outline-none focus-visible:outline-none"
      >
        {title}
      </h1>
      {sub && <p className="mt-2 text-sm leading-relaxed text-text-2">{sub}</p>}
      {left}
    </div>
    <div className="min-w-0">{right}</div>
  </div>
);

/** A row in a bordered box: an icon, a title with help, and controls. */
export const Row: React.FC<{
  icon: React.ReactNode;
  title: string;
  help?: React.ReactNode;
  aside?: React.ReactNode;
  tone?: "ok" | "warn";
  disabled?: boolean;
  children?: React.ReactNode;
}> = ({ icon, title, help, aside, tone, disabled, children }) => (
  <div
    className={`flex gap-3 border-b border-line p-4 last:border-b-0 ${
      tone === "warn" ? "bg-warn-soft" : ""
    } ${disabled ? "opacity-50" : ""}`}
  >
    <span
      className={`flex h-8 w-8 flex-none items-center justify-center rounded-[8px] ${
        tone === "warn"
          ? "bg-surface text-warn"
          : tone === "ok"
            ? "bg-teal-50 text-teal-700"
            : "bg-surface-2 text-text-2"
      }`}
      aria-hidden="true"
    >
      {icon}
    </span>
    <div className="min-w-0 flex-1">
      <div className="flex items-start justify-between gap-3">
        <b className="text-sm font-medium">{title}</b>
        {aside}
      </div>
      {help && <p className="mt-0.5 text-[13px] text-text-3">{help}</p>}
      {children}
    </div>
  </div>
);

/** A shortcut as key caps. */
export const Keys: React.FC<{ binding: string; className?: string }> = ({
  binding,
  className = "",
}) => (
  <span className={`inline-flex flex-wrap items-center gap-1 ${className}`}>
    {formatKeyCombination(binding)
      .split(" + ")
      .filter(Boolean)
      .map((key) => (
        <kbd key={key} className="kbd">
          {key}
        </kbd>
      ))}
  </span>
);

const BRAND: Record<Scene, { icon: typeof Mail; color: string; app: string }> =
  {
    message: { icon: MessageCircle, color: "#25d366", app: "WhatsApp" },
    email: { icon: Mail, color: "#0f6cbd", app: "Outlook" },
    list: { icon: FileText, color: "currentColor", app: "Notion" },
  };

/** The app a scene copies, by name. */
export const appOf = (scene: Scene) => BRAND[scene].app;

/** The scene's app as a small glyph in its colour. */
export const AppGlyph: React.FC<{ scene: Scene }> = ({ scene }) => {
  const { icon: Icon, color } = BRAND[scene];
  return (
    <Icon
      className="h-3.5 w-3.5 flex-none"
      style={{ color }}
      aria-hidden="true"
    />
  );
};
