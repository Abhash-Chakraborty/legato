import React from "react";
import type { LucideIcon } from "@/icons";
import type { Page } from "../../stores/navStore";
import { PAGE_MARKS, type Hue } from "./marks";

/** A page's title with its mark: the icon it has in the sidebar, on its
 *  own colour. `icon` and `hue` are for a part of a page with a mark of
 *  its own, such as a section of Settings. */
export const PageTitle: React.FC<{
  page: Page;
  icon?: LucideIcon;
  hue?: Hue;
  children: React.ReactNode;
}> = ({ page, icon, hue, children }) => {
  const Icon = icon ?? PAGE_MARKS[page].icon;
  return (
    <h1 className="flex min-w-0 items-center gap-3 text-[26px] font-semibold leading-tight tracking-[-0.015em]">
      <span
        className={`tile lg on tile-${hue ?? PAGE_MARKS[page].hue}`}
        aria-hidden="true"
      >
        <Icon className="h-[17px] w-[17px] stroke-[1.9]" />
      </span>
      <span className="min-w-0 truncate">{children}</span>
    </h1>
  );
};
