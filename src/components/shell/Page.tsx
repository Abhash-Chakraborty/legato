import React from "react";

interface PageProps {
  title: string;
  subtitle?: string;
  /** Sits at the right of the title, e.g. a feature's on/off switch. */
  actions?: React.ReactNode;
  /** Every page is centred in the content card; reading pages stop at
   *  1120 px, full pages at 1320 px. */
  width?: "reading" | "full";
  children: React.ReactNode;
}

/** A page inside the content card: a 26/600 title, one subtitle line, then
 *  the content. */
export const Page: React.FC<PageProps> = ({
  title,
  subtitle,
  actions,
  width = "reading",
  children,
}) => (
  <div
    className={`mx-auto w-full ${width === "reading" ? "max-w-[1120px]" : "max-w-[1320px]"}`}
  >
    <div className="mb-6 flex min-h-10 items-center justify-between gap-4">
      <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
        {title}
      </h1>
      {actions && <div className="flex items-center gap-2">{actions}</div>}
    </div>
    {subtitle && (
      <p className="-mt-3.5 mb-6 max-w-[640px] text-sm text-text-2">
        {subtitle}
      </p>
    )}
    {children}
  </div>
);
