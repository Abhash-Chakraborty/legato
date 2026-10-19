import React from "react";
import { useTranslation } from "react-i18next";
import {
  BookA,
  Cpu,
  House,
  Replace,
  Settings,
  type LucideIcon,
} from "lucide-react";
import legatoIcon from "../assets/legato-icon.svg";
import { useNavStore, type Page } from "../stores/navStore";

// A brand name, so it stays out of the translation files.
const APP_NAME = "Legato";

interface NavItem {
  id: Page;
  labelKey: string;
  icon: LucideIcon;
  /** Starts a new group, with space above it. */
  gapBefore?: boolean;
}

// What you do with dictation first, then the words and prompts you keep;
// configuration sits at the bottom. Pages from later steps of the plan
// (Snippets, Style, Notes, Scratchpad, Insights) join when they exist.
const TOP: NavItem[] = [
  { id: "home", labelKey: "sidebar.home", icon: House },
  {
    id: "dictionary",
    labelKey: "sidebar.dictionary",
    icon: BookA,
    gapBefore: true,
  },
  { id: "transforms", labelKey: "sidebar.transforms", icon: Replace },
];
const BOTTOM: NavItem[] = [
  { id: "models", labelKey: "sidebar.models", icon: Cpu },
  { id: "settings", labelKey: "sidebar.settings", icon: Settings },
];

const NavButton: React.FC<{ item: NavItem }> = ({ item }) => {
  const { t } = useTranslation();
  const page = useNavStore((state) => state.page);
  const go = useNavStore((state) => state.go);
  const current = page === item.id;
  const Icon = item.icon;

  return (
    <button
      type="button"
      onClick={() => go(item.id)}
      aria-current={current ? "page" : undefined}
      className={`group relative flex h-[38px] w-full items-center gap-3 rounded-[var(--r-md)] px-2.5 text-[15px] text-text transition-colors duration-[var(--dur-1)] hover:duration-0 focus-visible:outline-offset-[-2px] ${
        current
          ? "bg-sidebar-active font-medium"
          : "hover:bg-surface-3 active:bg-surface-4"
      } ${item.gapBefore ? "mt-3.5" : ""}`}
    >
      {current && (
        <span
          aria-hidden="true"
          className="absolute -start-3 top-2.5 bottom-2.5 w-[3px] rounded-e-[3px] bg-teal-700"
        />
      )}
      <Icon
        aria-hidden
        className={`h-[18px] w-[18px] flex-none stroke-[1.75] ${current ? "text-text" : "text-text-2 group-hover:text-text"}`}
      />
      <span className="min-w-0 flex-1 truncate text-start">
        {t(item.labelKey)}
      </span>
    </button>
  );
};

export const Sidebar: React.FC = () => {
  const { t } = useTranslation();

  return (
    <nav
      aria-label={t("sidebar.label")}
      className="flex min-h-0 flex-col gap-0.5 overflow-hidden px-3 pt-1 pb-4"
    >
      <div className="mb-3.5 flex h-11 items-center gap-2.5 px-2 text-xl font-semibold tracking-[-0.01em] whitespace-nowrap">
        <img
          src={legatoIcon}
          width={24}
          height={24}
          alt=""
          draggable={false}
          className="flex-none rounded-[7px]"
        />
        <span>{APP_NAME}</span>
      </div>
      {TOP.map((item) => (
        <NavButton key={item.id} item={item} />
      ))}
      <div className="flex-1" />
      {BOTTOM.map((item) => (
        <NavButton key={item.id} item={item} />
      ))}
    </nav>
  );
};
