import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  AudioLines,
  BookA,
  ChartColumn,
  Cpu,
  House,
  NotebookPen,
  PanelLeftClose,
  PanelLeftOpen,
  Replace,
  Scissors,
  Settings,
  Type,
  type LucideIcon,
} from "@/icons";
import type { AppSettings } from "@/bindings";
import { useSettings } from "../hooks/useSettings";
import { useNavStore, type Page } from "../stores/navStore";

interface NavItem {
  id: Page;
  labelKey: string;
  icon: LucideIcon;
  /** Starts a new group, with space above it. */
  gapBefore?: boolean;
  /** The Settings › Features switch the page goes with. */
  feature?: "notes_enabled" | "scratchpad_enabled" | "insights_enabled";
  /** The move its icon plays on hover (`.im-*` in controls.css). */
  motion: string;
  /** The page's own colour, behind its icon (`.tile-*` in play.css). */
  hue: string;
}

// What you do with dictation first, then the words and prompts you keep,
// then what you record and jot down, and how you use it; configuration sits
// at the bottom.
const TOP: NavItem[] = [
  {
    id: "home",
    labelKey: "sidebar.home",
    hue: "teal",
    icon: House,
    motion: "lift",
  },
  {
    id: "dictionary",
    labelKey: "sidebar.dictionary",
    hue: "violet",
    icon: BookA,
    motion: "tilt",
    gapBefore: true,
  },
  {
    id: "snippets",
    labelKey: "sidebar.snippets",
    hue: "rose",
    icon: Scissors,
    motion: "snip",
  },
  {
    id: "style",
    labelKey: "sidebar.style",
    hue: "amber",
    icon: Type,
    motion: "type",
  },
  {
    id: "transforms",
    labelKey: "sidebar.transforms",
    hue: "sky",
    icon: Replace,
    motion: "swap",
  },
  {
    id: "notes",
    labelKey: "sidebar.notes",
    hue: "green",
    icon: AudioLines,
    motion: "meter",
    gapBefore: true,
    feature: "notes_enabled",
  },
  {
    id: "scratchpad",
    labelKey: "sidebar.scratchpad",
    hue: "orange",
    icon: NotebookPen,
    motion: "write",
    feature: "scratchpad_enabled",
  },
  {
    id: "insights",
    labelKey: "sidebar.insights",
    hue: "indigo",
    icon: ChartColumn,
    motion: "bars",
    feature: "insights_enabled",
  },
];
const BOTTOM: NavItem[] = [
  {
    id: "models",
    labelKey: "sidebar.models",
    hue: "slate",
    icon: Cpu,
    motion: "chip",
  },
  {
    id: "settings",
    labelKey: "sidebar.settings",
    hue: "slate",
    icon: Settings,
    motion: "turn",
  },
];

/** Whether a page shows: its feature is on (settings saved before the
 *  switches existed count as on). */
const shown = (item: NavItem, settings: AppSettings | null) =>
  !item.feature || (settings?.[item.feature] ?? true);

/** Whether the i-th shown page starts a group: it does, or a hidden page
 *  between it and the one above did. */
const startsGroup = (top: NavItem[], i: number) => {
  const from = i === 0 ? 0 : TOP.indexOf(top[i - 1]) + 1;
  return TOP.slice(from, TOP.indexOf(top[i]) + 1).some((n) => n.gapBefore);
};

const NavButton: React.FC<{
  item: NavItem;
  gap: boolean;
  collapsed: boolean;
}> = ({ item, gap, collapsed }) => {
  const { t } = useTranslation();
  const page = useNavStore((state) => state.page);
  const go = useNavStore((state) => state.go);
  const current = page === item.id;
  const Icon = item.icon;
  const label = t(item.labelKey);

  return (
    <button
      type="button"
      onClick={() => go(item.id)}
      aria-current={current ? "page" : undefined}
      aria-label={collapsed ? label : undefined}
      title={collapsed ? label : undefined}
      className={`group relative flex h-10 w-full items-center gap-2.5 rounded-[var(--r-md)] px-[7px] text-[15px] text-text transition-colors duration-[var(--dur-1)] hover:duration-0 ring-inside ${
        current
          ? "bg-sidebar-active font-medium"
          : "hover:bg-surface-3 active:bg-surface-4"
      } ${gap ? "mt-3.5" : ""}`}
    >
      {current && (
        <span
          aria-hidden="true"
          className="nav-mark absolute -start-3 top-2.5 bottom-2.5 w-[3px] rounded-e-[3px] bg-teal-700"
        />
      )}
      {/* Each page has a colour of its own; the page you're on wears it. */}
      <span className={`tile tile-${item.hue} ${current ? "on" : ""}`}>
        <Icon
          aria-hidden
          className={`im im-${item.motion} h-4 w-4 flex-none stroke-[1.9]`}
        />
      </span>
      {!collapsed && (
        <span className="min-w-0 flex-1 truncate text-start">{label}</span>
      )}
    </button>
  );
};

export const Sidebar: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const page = useNavStore((state) => state.page);
  const go = useNavStore((state) => state.go);
  const collapsed = useNavStore((state) => state.sidebarCollapsed);
  const toggle = useNavStore((state) => state.toggleSidebar);
  const top = TOP.filter((item) => shown(item, settings));

  // A page whose feature was just switched off goes back to Home.
  const hidden = TOP.some((item) => item.id === page && !shown(item, settings));
  useEffect(() => {
    if (hidden) go("home");
  }, [hidden, go]);

  return (
    <nav
      aria-label={t("sidebar.label")}
      className="flex min-h-0 flex-col gap-0.5 overflow-hidden px-3 pt-1 pb-4"
    >
      {top.map((item, i) => (
        <NavButton
          key={item.id}
          item={item}
          gap={startsGroup(top, i)}
          collapsed={collapsed}
        />
      ))}
      <div className="flex-1" />
      {BOTTOM.map((item) => (
        <NavButton
          key={item.id}
          item={item}
          gap={!!item.gapBefore}
          collapsed={collapsed}
        />
      ))}
      <button
        type="button"
        onClick={toggle}
        aria-expanded={!collapsed}
        aria-label={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
        title={collapsed ? t("sidebar.expand") : t("sidebar.collapse")}
        className="group mt-1 flex h-[34px] w-full items-center gap-3 rounded-[var(--r-md)] px-2.5 text-[13px] text-text-3 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 active:bg-surface-4 ring-inside"
      >
        {collapsed ? (
          <PanelLeftOpen
            aria-hidden
            className="h-[18px] w-[18px] flex-none stroke-[1.75]"
          />
        ) : (
          <PanelLeftClose
            aria-hidden
            className="h-[18px] w-[18px] flex-none stroke-[1.75]"
          />
        )}
        {!collapsed && (
          <span className="min-w-0 flex-1 truncate text-start">
            {t("sidebar.collapse")}
          </span>
        )}
      </button>
    </nav>
  );
};
