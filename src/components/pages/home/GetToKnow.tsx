import React, { useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  BookA,
  CalendarDays,
  Check,
  ChevronDown,
  ChevronUp,
  NotebookPen,
  Pencil,
  Plug,
  Replace,
  Scissors,
  Smartphone,
  SquarePen,
  Type,
  X,
  type LucideIcon,
} from "@/icons";
import { commands } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useAiStore, watchAi } from "../../../stores/aiStore";
import {
  useNavStore,
  type Page,
  type SettingsSection,
} from "../../../stores/navStore";
import { formatKeyCombination } from "../../../lib/utils/keyboard";

const KEY = "legato.tour";

/** What this PC remembers of the list: hidden for good, and the rows
 *  marked done by hand. */
interface Kept {
  hidden: boolean;
  done: string[];
}

const read = (): Kept => {
  try {
    const kept = JSON.parse(localStorage.getItem(KEY) ?? "{}") as Partial<Kept>;
    return {
      hidden: kept.hidden === true,
      done: Array.isArray(kept.done) ? kept.done : [],
    };
  } catch {
    return { hidden: false, done: [] };
  }
};

const write = (kept: Kept) => {
  try {
    localStorage.setItem(KEY, JSON.stringify(kept));
  } catch {
    // Shown again next time; nothing else depends on it.
  }
};

/** Shows the list on Home again, from Settings › About. */
export const showTourAgain = () => write({ hidden: false, done: [] });

/** How much of Legato is in use, counted once when Home opens. */
interface Used {
  words: number;
  snippets: number;
  notes: number;
  scratch: number;
  phones: number;
}

type Feature =
  | "dictionary"
  | "correct"
  | "style"
  | "ai"
  | "transforms"
  | "snippets"
  | "notes"
  | "scratchpad"
  | "phone"
  | "calendar";

interface Row {
  id: Feature;
  icon: LucideIcon;
  /** Where Show me goes. */
  page: Page;
  section?: SettingsSection;
  /** The shortcut its text names, by binding. */
  binding?: string;
  /** Legato can tell it's in use; without, the row is ticked by hand. */
  used?: boolean;
  /** Left out while its feature is switched off. */
  on: boolean;
}

/**
 * What Legato can do beyond dictation, one thing at a time: the next
 * feature you haven't tried, what it's for and where it is. A row ticks
 * itself once the feature is in use, or you tick it; All lists the rest.
 */
export const GetToKnow: React.FC = () => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const go = useNavStore((s) => s.go);
  const ai = useAiStore((s) => s.status);
  useEffect(watchAi, []);
  const [kept, setKept] = useState<Kept>(read);
  const [used, setUsed] = useState<Used | null>(null);
  const [all, setAll] = useState(false);

  useEffect(() => {
    if (kept.hidden) return;
    let live = true;
    const count = async <T,>(
      ask: () => Promise<{ status: string; data?: T }>,
      size: (data: T) => number,
    ) => {
      const result = await ask().catch(() => null);
      return result?.status === "ok" && result.data !== undefined
        ? size(result.data)
        : 0;
    };
    Promise.all([
      count(commands.getDictionary, (d) => d.entries.length),
      count(commands.getSnippets, (s) => s.snippets.length),
      count(commands.getNotes, (n) => n.length),
      count(commands.getScratchNotes, (n) => n.length),
      count(commands.getSyncState, (s) => s.phones.length),
    ]).then(([words, snippets, notes, scratch, phones]) => {
      if (live) setUsed({ words, snippets, notes, scratch, phones });
    });
    return () => {
      live = false;
    };
  }, [kept.hidden]);

  const rows = useMemo<Row[]>(() => {
    if (!settings || !used) return [];
    const transforms = settings.post_process_enabled ?? false;
    return (
      [
        {
          id: "dictionary",
          icon: BookA,
          page: "dictionary",
          used: used.words > 0,
          on: true,
        },
        {
          id: "correct",
          icon: Pencil,
          page: "settings",
          section: "shortcuts",
          binding: "correct_last",
          on: true,
        },
        { id: "style", icon: Type, page: "style", on: true },
        {
          id: "ai",
          icon: Plug,
          page: "settings",
          section: "ai",
          used: !!ai && ai.kind !== "none",
          on: true,
        },
        {
          id: "transforms",
          icon: Replace,
          page: "transforms",
          binding: "transform_selection",
          on: transforms,
        },
        {
          id: "snippets",
          icon: Scissors,
          page: "snippets",
          used: used.snippets > 0,
          on: true,
        },
        {
          id: "notes",
          icon: NotebookPen,
          page: "notes",
          binding: "toggle_notes",
          used: used.notes > 0,
          on: settings.notes_enabled ?? true,
        },
        {
          id: "scratchpad",
          icon: SquarePen,
          page: "scratchpad",
          binding: "open_scratchpad",
          used: used.scratch > 0,
          on: settings.scratchpad_enabled ?? true,
        },
        {
          id: "phone",
          icon: Smartphone,
          page: "settings",
          section: "devices",
          used: used.phones > 0,
          on: true,
        },
        {
          id: "calendar",
          icon: CalendarDays,
          page: "settings",
          section: "connections",
          used: (settings.calendars?.length ?? 0) > 0,
          on: settings.notes_enabled ?? true,
        },
      ] satisfies Row[]
    ).filter((row) => row.on);
  }, [settings, used, ai]);

  if (kept.hidden || rows.length === 0) return null;

  const isDone = (row: Row) => row.used === true || kept.done.includes(row.id);
  const left = rows.filter((row) => !isDone(row));
  // Everything tried: nothing more to show.
  if (left.length === 0) return null;

  const keep = (next: Kept) => {
    setKept(next);
    write(next);
  };
  const tick = (row: Row) =>
    keep({ ...kept, done: [...new Set([...kept.done, row.id])] });
  const keys = (row: Row) =>
    formatKeyCombination(
      (row.binding && settings?.bindings?.[row.binding]?.current_binding) || "",
    );
  const body = (row: Row) => {
    const shortcut = keys(row);
    return t(
      shortcut
        ? `home.tour.rows.${row.id}.body`
        : [
            `home.tour.rows.${row.id}.bodyNoKeys`,
            `home.tour.rows.${row.id}.body`,
          ],
      { keys: shortcut },
    );
  };
  const show = (row: Row) => {
    // Seen is as much as Legato can know for a row it can't count.
    if (row.used === undefined) tick(row);
    go(row.page, row.section);
  };
  const next = left[0];
  const done = rows.length - left.length;

  return (
    <section
      aria-label={t("home.tour.title")}
      className="mb-5 overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface"
    >
      <div className="flex min-h-10 items-center gap-2 border-b border-line ps-3.5 pe-1.5">
        <h2 className="text-[13px] font-semibold">{t("home.tour.title")}</h2>
        <span className="text-[13px] tabular-nums text-text-3">
          {t("home.tour.progress", { done, count: rows.length })}
        </span>
        <span className="flex-1" />
        <button
          type="button"
          onClick={() => setAll(!all)}
          aria-expanded={all}
          className="flex h-7 items-center gap-1 rounded-[var(--r-xs)] px-2 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0"
        >
          {all ? t("home.tour.less") : t("home.tour.all")}
          {all ? (
            <ChevronUp className="h-3.5 w-3.5" aria-hidden="true" />
          ) : (
            <ChevronDown className="h-3.5 w-3.5" aria-hidden="true" />
          )}
        </button>
        <button
          type="button"
          onClick={() => keep({ ...kept, hidden: true })}
          aria-label={t("home.tour.hide")}
          title={t("home.tour.hide")}
          className="grid h-7 w-7 place-items-center rounded-[var(--r-xs)] text-text-3 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0"
        >
          <X className="h-3.5 w-3.5" aria-hidden="true" />
        </button>
      </div>
      <ul className="divide-y divide-line">
        {(all ? rows : [next]).map((row) => {
          const ticked = isDone(row);
          const Icon = row.icon;
          return (
            <li key={row.id} className="flex items-start gap-3 px-3.5 py-3">
              <span
                className={`mt-0.5 grid h-7 w-7 flex-none place-items-center rounded-[var(--r-sm)] ${ticked ? "bg-teal-50 text-teal-700" : "bg-surface-2 text-text-2"}`}
              >
                {ticked ? (
                  <Check className="h-4 w-4" aria-hidden="true" />
                ) : (
                  <Icon className="h-4 w-4" aria-hidden="true" />
                )}
              </span>
              <div className="min-w-0 flex-1">
                <h3 className="text-sm font-medium">
                  {t(`home.tour.rows.${row.id}.title`)}
                  {ticked && (
                    <span className="ms-2 text-[12.5px] font-normal text-text-3">
                      {t("home.tour.done")}
                    </span>
                  )}
                </h3>
                <p className="mt-0.5 text-[13px] leading-snug text-text-2">
                  {body(row)}
                </p>
              </div>
              <div className="flex flex-none items-center gap-1">
                {!ticked && row.used !== undefined && (
                  <button
                    type="button"
                    onClick={() => tick(row)}
                    className="h-8 rounded-[var(--r-sm)] px-2.5 text-[13px] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0"
                  >
                    {t("home.tour.skip")}
                  </button>
                )}
                <button
                  type="button"
                  onClick={() => show(row)}
                  className="h-8 rounded-[var(--r-sm)] border border-line-strong bg-surface px-3 text-[13px] font-medium transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0 active:scale-[0.97]"
                >
                  {t("home.tour.show")}
                </button>
              </div>
            </li>
          );
        })}
      </ul>
    </section>
  );
};
