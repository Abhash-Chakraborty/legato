import React, { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { Search } from "lucide-react";
import { commands, type Note } from "@/bindings";
import { isYou, taskGroups, whenLabel, type TaskShow } from "../../notes/notes";
import { Titled } from "./SummaryTab";

const SHOWS: TaskShow[] = ["mine", "everyone", "done"];

/** Next steps from every note in one list, by note: yours still open,
 *  everyone's, or the ones done. A note's title opens it. */
export const TasksView: React.FC<{
  notes: Note[];
  onOpen: (id: number) => void;
}> = ({ notes, onOpen }) => {
  const { t, i18n } = useTranslation();
  const [show, setShow] = useState<TaskShow>("mine");
  const [query, setQuery] = useState("");
  // Ticked here before the store catches up, so a row doesn't jump back.
  const [done, setDone] = useState<Record<number, boolean>>({});
  const now = useMemo(() => new Date(), []);

  const groups = useMemo(
    () => taskGroups(notes, show, query),
    [notes, show, query],
  );
  const counts = useMemo(
    () =>
      Object.fromEntries(
        SHOWS.map((s) => [
          s,
          taskGroups(notes, s, "").reduce((n, g) => n + g.actions.length, 0),
        ]),
      ) as Record<TaskShow, number>,
    [notes],
  );

  const toggle = async (id: number, next: boolean) => {
    setDone((d) => ({ ...d, [id]: next }));
    const result = await commands.setNoteActionDone(id, next).catch(() => null);
    if (result?.status !== "ok") setDone((d) => ({ ...d, [id]: !next }));
  };

  return (
    <div className="flex min-h-0 flex-col overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface min-[1000px]:flex-1">
      <div className="flex flex-wrap items-center gap-2.5 border-b border-line p-3">
        <div className="relative min-w-[200px] flex-1">
          <Search
            className="pointer-events-none absolute start-[11px] top-1/2 h-[14px] w-[14px] -translate-y-1/2 text-text-3"
            aria-hidden="true"
          />
          <input
            type="search"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Escape" && query) {
                e.preventDefault();
                e.stopPropagation();
                setQuery("");
              }
            }}
            placeholder={t("notes.tasks.search")}
            aria-label={t("notes.tasks.search")}
            className="input w-full ps-[32px] [&::-webkit-search-cancel-button]:hidden"
          />
        </div>
        <div
          role="radiogroup"
          aria-label={t("notes.filter.label")}
          className="inline-flex rounded-[var(--r-md)] bg-surface-2 p-[3px]"
        >
          {SHOWS.map((s) => (
            <button
              key={s}
              type="button"
              role="radio"
              aria-checked={show === s}
              onClick={() => {
                setShow(s);
                setDone({});
              }}
              className={`inline-flex h-[26px] items-center gap-1.5 rounded-[var(--r-sm)] px-2.5 text-xs transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                show === s
                  ? "bg-surface font-medium text-text shadow-sm"
                  : "text-text-2 hover:text-text"
              }`}
            >
              {t(`notes.tasks.show.${s}`)}
              <span className="tabular-nums text-text-3">{counts[s]}</span>
            </button>
          ))}
        </div>
      </div>

      {groups.length === 0 ? (
        <div className="px-7 py-14 text-center">
          <p className="text-sm font-medium">
            {query.trim()
              ? t("notes.tasks.noResults", { query: query.trim() })
              : t(`notes.tasks.empty.${show}`)}
          </p>
          <p className="mt-1 text-[13px] text-text-3">
            {query.trim()
              ? t("notes.tasks.noResultsBody")
              : t("notes.tasks.emptyBody")}
          </p>
        </div>
      ) : (
        <div className="min-h-0 flex-1 overflow-y-auto px-4 pb-2">
          {groups.map(({ note, actions }) => (
            <section key={note.id} className="pt-4">
              <h3 className="flex items-baseline gap-2 text-[13px]">
                <button
                  type="button"
                  onClick={() => onOpen(note.id)}
                  title={t("notes.tasks.open")}
                  className="min-w-0 truncate font-medium text-text underline-offset-2 hover:underline"
                >
                  {note.title}
                </button>
                <span className="flex-none text-xs text-text-3">
                  {whenLabel(note.started_at, now, i18n.language)}
                </span>
              </h3>
              <ul className="mt-1">
                {actions.map((a) => {
                  const ticked = done[a.id] ?? a.done;
                  return (
                    <li key={a.id}>
                      <label className="flex min-h-10 items-center gap-3 border-b border-line py-2 text-sm last:border-b-0">
                        <input
                          type="checkbox"
                          className="check"
                          checked={ticked}
                          onChange={(e) => toggle(a.id, e.target.checked)}
                        />
                        <span
                          className={`min-w-0 flex-1 ${ticked ? "text-text-3 line-through" : ""}`}
                        >
                          {ticked ? a.text : <Titled text={a.text} />}
                        </span>
                        <span
                          className={`flex-none rounded-[var(--r-xs)] px-1.5 py-0.5 text-xs ${
                            isYou(a.owner)
                              ? "bg-teal-50 text-teal-700"
                              : "bg-surface-2 text-text-2"
                          }`}
                        >
                          {isYou(a.owner) ? t("notes.who.you") : a.owner}
                        </span>
                      </label>
                    </li>
                  );
                })}
              </ul>
            </section>
          ))}
        </div>
      )}
    </div>
  );
};
