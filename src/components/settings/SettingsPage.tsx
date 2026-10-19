import React, {
  useEffect,
  useLayoutEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { useTranslation } from "react-i18next";
import { Check, Search, X } from "lucide-react";
import { useSettings } from "../../hooks/useSettings";
import { useSettingsStore } from "../../stores/settingsStore";
import { useModelStore } from "../../stores/modelStore";
import { useNavStore, type SettingsSection } from "../../stores/navStore";
import { SettingsGroup } from "../ui/SettingsGroup";
import { Requires } from "../ui/SettingContainer";
import type { OnboardingPreviewStep } from "./debug/OnboardingPreview";
import {
  buildSections,
  type RowContext,
  type SectionDef,
  type SettingRow,
} from "./sections";

const HIGHLIGHT = "settings-search";

/** Paints every match of `query` inside the rows' titles and help with the
 *  CSS Custom Highlight API, so the rows' markup stays as it is. */
const useSearchHighlight = (
  root: React.RefObject<HTMLElement>,
  query: string,
) => {
  useLayoutEffect(() => {
    const registry = (CSS as unknown as { highlights?: Map<string, unknown> })
      .highlights;
    const HighlightCtor = (
      window as unknown as { Highlight?: new (...r: Range[]) => unknown }
    ).Highlight;
    if (!registry || !HighlightCtor) return;
    registry.delete(HIGHLIGHT);
    const q = query.trim().toLowerCase();
    if (!q || !root.current) return;

    const ranges: Range[] = [];
    root.current
      .querySelectorAll("[data-setting-title], [data-setting-help]")
      .forEach((el) => {
        const walker = document.createTreeWalker(el, NodeFilter.SHOW_TEXT);
        for (let node = walker.nextNode(); node; node = walker.nextNode()) {
          const text = node.textContent?.toLowerCase() ?? "";
          for (
            let i = text.indexOf(q);
            i >= 0;
            i = text.indexOf(q, i + q.length)
          ) {
            const range = new Range();
            range.setStart(node, i);
            range.setEnd(node, i + q.length);
            ranges.push(range);
          }
        }
      });
    registry.set(HIGHLIGHT, new HighlightCtor(...ranges));
    return () => {
      registry.delete(HIGHLIGHT);
    };
  });
};

/** Wraps a row in its dependency, so it greys out and says why. */
const Row: React.FC<{ row: SettingRow; ctx: RowContext }> = ({ row, ctx }) => {
  const { t } = useTranslation();
  const unmet = row.requires?.(ctx) ?? null;
  return (
    <Requires
      met={unmet === null}
      reason={unmet ? t(unmet.reasonKey, unmet.values) : ""}
    >
      {row.render()}
    </Requires>
  );
};

/** Flashes "Saved" for a moment after each change that saved. */
const SavedTick: React.FC = () => {
  const { t } = useTranslation();
  const savedAt = useSettingsStore((state) => state.savedAt);
  const [show, setShow] = useState(false);
  const first = useRef(savedAt);

  useEffect(() => {
    if (savedAt === first.current) return;
    setShow(true);
    const timer = setTimeout(() => setShow(false), 1600);
    return () => clearTimeout(timer);
  }, [savedAt]);

  return (
    <span className={`saved ${show ? "show" : ""}`} role="status">
      {show && (
        <>
          <Check
            className="h-[13px] w-[13px] stroke-[2.5]"
            aria-hidden="true"
          />
          {t("settings.saved")}
        </>
      )}
    </span>
  );
};

interface SettingsPageProps {
  onPreviewOnboarding: (step: OnboardingPreviewStep) => void;
}

export const SettingsPage: React.FC<SettingsPageProps> = ({
  onPreviewOnboarding,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const { currentModel, models } = useModelStore();
  const section = useNavStore((state) => state.settingsSection);
  const go = useNavStore((state) => state.go);
  const [query, setQuery] = useState("");
  const contentRef = useRef<HTMLDivElement>(null);
  const searchRef = useRef<HTMLInputElement>(null);

  const sections = useMemo(
    () => buildSections(onPreviewOnboarding),
    [onPreviewOnboarding],
  );
  const visible = sections.filter((s) => !s.hidden?.(settings));
  const current: SectionDef =
    visible.find((s) => s.id === section) ?? visible[0];

  // Debug can disappear while open (Turn off); fall back to General.
  useEffect(() => {
    if (current.id !== section) go("settings", current.id);
  }, [current.id, section, go]);

  const ctx: RowContext = {
    settings,
    model: models.find((m) => m.id === currentModel),
  };

  const q = query.trim().toLowerCase();
  const matches = (row: SettingRow) =>
    row.searchKeys.some((key) => t(key).toLowerCase().includes(q));
  const results = q
    ? visible
        .map((s) => ({
          section: s,
          rows: s.groups.flatMap((g) => g.rows.filter(matches)),
        }))
        .filter((r) => r.rows.length > 0)
    : [];

  useSearchHighlight(contentRef, query);

  // Keep the page at the top when switching sections.
  useLayoutEffect(() => {
    contentRef.current?.closest("[data-scroll-root]")?.scrollTo({ top: 0 });
  }, [section, q]);

  const openSection = (id: SettingsSection) => {
    setQuery("");
    go("settings", id);
  };

  // Up and Down move between sections in the list, like a Windows nav pane.
  const onNavKeyDown = (e: React.KeyboardEvent<HTMLDivElement>) => {
    if (e.key !== "ArrowDown" && e.key !== "ArrowUp") return;
    e.preventDefault();
    // React clears currentTarget once the handler returns.
    const list = e.currentTarget;
    const ids = visible.map((s) => s.id);
    const i = ids.indexOf(current.id);
    const next =
      ids[(i + (e.key === "ArrowDown" ? 1 : -1) + ids.length) % ids.length];
    openSection(next);
    requestAnimationFrame(() =>
      list
        .querySelector<HTMLButtonElement>(`[data-section="${next}"]`)
        ?.focus(),
    );
  };

  return (
    <div className="flex items-start gap-8 min-[1180px]:gap-12">
      <div className="sticky top-0 w-[168px] flex-none min-[1180px]:w-[184px]">
        <div className="relative mb-3">
          <Search
            className="pointer-events-none absolute start-[11px] top-1/2 h-[15px] w-[15px] -translate-y-1/2 text-text-3"
            aria-hidden="true"
          />
          <input
            ref={searchRef}
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
            placeholder={t("settings.search.placeholder")}
            aria-label={t("settings.search.placeholder")}
            className="input compact ps-[33px] pe-8 [&::-webkit-search-cancel-button]:hidden"
          />
          {query && (
            <button
              type="button"
              onClick={() => {
                setQuery("");
                searchRef.current?.focus();
              }}
              aria-label={t("settings.search.clear")}
              title={t("settings.search.clear")}
              className="absolute end-1 top-1/2 grid h-6 w-6 -translate-y-1/2 place-items-center rounded-[var(--r-xs)] text-text-3 hover:bg-surface-3 hover:text-text"
            >
              <X className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
          )}
        </div>
        <div
          role="navigation"
          aria-label={t("settings.sectionsLabel")}
          className="flex flex-col gap-px"
          onKeyDown={onNavKeyDown}
        >
          {visible.map((s) => {
            const Icon = s.icon;
            const on = !q && s.id === current.id;
            return (
              <button
                key={s.id}
                type="button"
                data-section={s.id}
                onClick={() => openSection(s.id)}
                aria-current={on ? "true" : undefined}
                className={`group flex h-[34px] w-full items-center gap-2.5 rounded-[var(--r-sm)] px-2.5 text-sm transition-colors duration-[var(--dur-1)] hover:duration-0 focus-visible:outline-offset-[-2px] ${
                  on
                    ? "bg-surface-2 font-medium text-text"
                    : "text-text-2 hover:bg-surface-2 hover:text-text"
                }`}
              >
                <Icon
                  aria-hidden
                  className={`h-4 w-4 flex-none stroke-[1.75] ${on ? "text-text" : "text-text-3 group-hover:text-text-2"}`}
                />
                <span className="truncate">
                  {t(`settings.sections.${s.id}.title`)}
                </span>
              </button>
            );
          })}
        </div>
      </div>

      <div ref={contentRef} className="min-w-0 max-w-[760px] flex-1">
        {q ? (
          <>
            <div className="mb-6 flex min-h-10 items-center justify-between gap-4">
              <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
                {t("settings.search.title")}
              </h1>
              <SavedTick />
            </div>
            {results.length === 0 ? (
              <div className="grid place-items-center gap-1.5 px-6 py-12 text-center">
                <h2 className="text-[15px] font-medium">
                  {t("settings.search.noResults", { query: query.trim() })}
                </h2>
                <p className="text-[13px] text-text-3">
                  {t("settings.search.noResultsHelp")}
                </p>
              </div>
            ) : (
              results.map(({ section: s, rows }) => (
                <SettingsGroup
                  key={s.id}
                  title={t(`settings.sections.${s.id}.title`)}
                >
                  {rows.map((row) => (
                    <Row key={row.id} row={row} ctx={ctx} />
                  ))}
                </SettingsGroup>
              ))
            )}
          </>
        ) : (
          <div key={current.id} className="view-enter">
            <div className="mb-6 flex min-h-10 items-center justify-between gap-4">
              <h1 className="text-[26px] font-semibold leading-tight tracking-[-0.015em]">
                {t(`settings.sections.${current.id}.title`)}
              </h1>
              <SavedTick />
            </div>
            <p className="-mt-3.5 mb-6 max-w-[640px] text-sm text-text-2">
              {t(`settings.sections.${current.id}.subtitle`)}
            </p>
            {current.groups.map((g) => (
              <SettingsGroup key={g.id} title={t(g.titleKey)}>
                {g.rows.map((row) => (
                  <Row key={row.id} row={row} ctx={ctx} />
                ))}
              </SettingsGroup>
            ))}
          </div>
        )}
      </div>
    </div>
  );
};
