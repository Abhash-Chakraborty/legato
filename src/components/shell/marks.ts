import {
  AudioLines,
  BookA,
  ChartColumn,
  Cpu,
  House,
  NotebookPen,
  Replace,
  Scissors,
  Settings,
  Type,
  type LucideIcon,
} from "@/icons";
import { useNavStore, type Page } from "../../stores/navStore";

/** A colour from play.css: `.tile-<hue>` behind an icon, `data-hue` on a
 *  part of the page that takes it as its accent. */
export type Hue =
  | "teal"
  | "violet"
  | "rose"
  | "amber"
  | "sky"
  | "green"
  | "orange"
  | "indigo"
  | "slate";

/** Each page's icon and its own colour: in the sidebar, beside its title,
 *  and as the accent of its buttons, switches, tabs and links. Home keeps
 *  Legato's teal; Models and Settings are tools, in slate, and keep the
 *  teal accent (Settings colours each of its sections instead). */
export const PAGE_MARKS: Record<Page, { icon: LucideIcon; hue: Hue }> = {
  home: { icon: House, hue: "teal" },
  dictionary: { icon: BookA, hue: "violet" },
  snippets: { icon: Scissors, hue: "rose" },
  style: { icon: Type, hue: "amber" },
  transforms: { icon: Replace, hue: "sky" },
  notes: { icon: AudioLines, hue: "green" },
  scratchpad: { icon: NotebookPen, hue: "orange" },
  insights: { icon: ChartColumn, hue: "indigo" },
  models: { icon: Cpu, hue: "slate" },
  settings: { icon: Settings, hue: "slate" },
};

/** The colour of the page that's open, for dialogs and sheets it opens:
 *  they are drawn outside the page, so they ask. */
export const usePageHue = (): Hue =>
  useNavStore((state) => PAGE_MARKS[state.page].hue);
