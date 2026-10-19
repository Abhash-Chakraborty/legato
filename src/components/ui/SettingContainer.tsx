import React, { createContext, useContext, useEffect, useRef } from "react";

interface SettingContainerProps {
  title: string;
  description: React.ReactNode;
  children: React.ReactNode;
  /** Kept for existing callers; help text now always shows under the title. */
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  layout?: "horizontal" | "stacked";
  disabled?: boolean;
  tooltipPosition?: "top" | "bottom";
}

/**
 * Why the rows inside a <Requires> can't be changed right now. A setting that
 * depends on another (Start hidden needs the tray icon) greys out and shows
 * this in place of its help text, without each setting component knowing.
 */
const RequiresContext = createContext<string | null>(null);

export const Requires: React.FC<{
  /** Whether the dependency is met. */
  met: boolean;
  reason: string;
  children: React.ReactNode;
}> = ({ met, reason, children }) => (
  <RequiresContext.Provider value={met ? null : reason}>
    {children}
  </RequiresContext.Provider>
);

/** One settings row: title and help on the left, the control on the right. */
export const SettingContainer: React.FC<SettingContainerProps> = ({
  title,
  description,
  children,
  layout = "horizontal",
  disabled = false,
}) => {
  const reason = useContext(RequiresContext);
  const blocked = reason !== null;
  const dim = disabled || blocked;
  const controlRef = useRef<HTMLDivElement>(null);

  // A blocked row's control can't be focused or clicked. React 18 has no
  // `inert` prop, so set the attribute directly.
  useEffect(() => {
    controlRef.current?.toggleAttribute("inert", blocked);
  }, [blocked]);

  // In a horizontal row the text keeps at least 240px; a narrower row puts
  // the control on its own line below.
  const text = (
    <div
      className={layout === "stacked" ? "min-w-0" : "min-w-0 flex-[1_1_240px]"}
    >
      <h3
        data-setting-title
        className={`text-[15px] font-medium leading-snug transition-opacity duration-[var(--dur-2)] ${dim ? "opacity-45" : ""}`}
      >
        {title}
      </h3>
      {blocked ? (
        <p className="mt-0.5 max-w-[520px] text-[13px] leading-[1.45] text-text-2">
          {reason}
        </p>
      ) : (
        description && (
          <p
            data-setting-help
            className={`mt-0.5 max-w-[520px] text-[13px] leading-[1.45] text-text-3 ${dim ? "opacity-60" : ""}`}
          >
            {description}
          </p>
        )
      )}
    </div>
  );

  if (layout === "stacked") {
    return (
      <div data-setting-row className="py-3.5">
        <div className="mb-3">{text}</div>
        <div
          ref={controlRef}
          className={`w-full transition-opacity duration-[var(--dur-2)] ${blocked ? "opacity-45" : ""}`}
        >
          {children}
        </div>
      </div>
    );
  }

  return (
    <div
      data-setting-row
      className="flex min-h-[60px] flex-wrap items-center justify-between gap-x-8 gap-y-3 py-3.5"
    >
      {text}
      <div
        ref={controlRef}
        className={`relative flex flex-none items-center gap-2.5 transition-opacity duration-[var(--dur-2)] ${blocked ? "opacity-45" : ""}`}
      >
        {children}
      </div>
    </div>
  );
};
