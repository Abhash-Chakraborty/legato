import React, { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Crosshair, Globe, RotateCcw, Search, X } from "lucide-react";
import { toast } from "sonner";
import {
  commands,
  type AppCategory,
  type AppRule,
  type StyleApp,
} from "@/bindings";
import { useTextHighlight } from "../../../hooks/useTextHighlight";
import { Button } from "../../ui/Button";
import { Sheet } from "../../ui/Sheet";
import { AppIcon } from "../home/AppIcon";

const PICK_SECONDS = 3;

const keyOf = (a: StyleApp) => `${a.rule.kind}:${a.rule.pattern}`;

const matches = (a: StyleApp, q: string) => {
  const query = q.trim().toLowerCase();
  return (
    !query ||
    a.name.toLowerCase().includes(query) ||
    a.exe.toLowerCase().includes(query) ||
    (a.site ?? "").toLowerCase().includes(query)
  );
};

const AppRow: React.FC<{
  app: StyleApp;
  detail: string;
  action: React.ReactNode;
}> = ({ app, detail, action }) => (
  <li className="flex items-center gap-3 border-b border-line px-3 py-2.5 last:border-b-0">
    {app.exe_path ? (
      <AppIcon path={app.exe_path} className="h-6 w-6" />
    ) : (
      <Globe className="h-6 w-6 p-0.5 text-text-3" aria-hidden="true" />
    )}
    <div className="min-w-0 flex-1">
      <div data-app-name className="truncate text-sm font-medium">
        {app.name}
      </div>
      <div className="truncate text-xs text-text-3">{detail}</div>
    </div>
    {action}
  </li>
);

interface EditAppsProps {
  category: AppCategory;
  apps: StyleApp[];
  onClose: () => void;
  /** Called with the saved rules after every change. */
  onRules: (rules: AppRule[]) => void;
}

/** Which apps and websites use one kind of app's style. Changes save as you
 *  go. */
export const EditApps: React.FC<EditAppsProps> = ({
  category,
  apps,
  onClose,
  onRules,
}) => {
  const { t, i18n } = useTranslation();
  const searchRef = useRef<HTMLInputElement>(null);
  const listRef = useRef<HTMLDivElement>(null);
  const siteId = useId();
  const [query, setQuery] = useState("");
  const [site, setSite] = useState("");
  const [siteError, setSiteError] = useState<string | null>(null);
  const [countdown, setCountdown] = useState<number | null>(null);
  const [picked, setPicked] = useState<StyleApp | null>(null);
  const [pickMissed, setPickMissed] = useState(false);
  const categoryName = t(`style.categories.${category}`);

  useTextHighlight(
    listRef,
    "apps-search",
    query.trim() ? [query.trim()] : [],
    "[data-app-name]",
  );

  useEffect(() => {
    if (countdown === null || countdown <= 0) return;
    const timer = setTimeout(() => setCountdown(countdown - 1), 1000);
    return () => clearTimeout(timer);
  }, [countdown]);

  const here = apps.filter((a) => a.category === category && matches(a, query));
  const others = apps.filter(
    (a) => a.category !== category && matches(a, query),
  );

  const move = async (app: StyleApp, to: AppCategory) => {
    const rules = await commands.moveApp(app.rule, to).catch(() => null);
    if (!rules) {
      toast.error(t("style.edit.moveFailed"));
      return;
    }
    onRules(rules);
    if (picked && keyOf(picked) === keyOf(app))
      setPicked({ ...picked, category: to });
  };

  const addSite = async () => {
    const result = await commands.addSite(site, category);
    if (result.status === "error") {
      setSiteError(result.error);
      return;
    }
    setSite("");
    setSiteError(null);
    onRules(result.data);
  };

  const pick = async () => {
    setPicked(null);
    setPickMissed(false);
    setCountdown(PICK_SECONDS);
    const app = await commands
      .pickWindow(PICK_SECONDS * 1000)
      .catch(() => null);
    setCountdown(null);
    if (app) setPicked(app);
    else setPickMissed(true);
  };

  const restore = async () => {
    const rules = await commands.restoreAppDefaults(category).catch(() => null);
    if (!rules) return;
    onRules(rules);
    toast.success(t("style.edit.restored", { category: categoryName }));
  };

  const detailOf = (a: StyleApp) =>
    [a.exe || null, a.site].filter(Boolean).join(" · ") ||
    t("style.edit.website");

  return (
    <Sheet
      open
      title={t("style.edit.title", { category: categoryName })}
      description={t("style.edit.description")}
      onClose={onClose}
      onSubmit={onClose}
      closeLabel={t("style.edit.done")}
      initialFocusRef={searchRef}
      footer={
        <>
          <Button variant="ghost" className="me-auto" onClick={restore}>
            <RotateCcw className="h-4 w-4" aria-hidden="true" />
            {t("style.edit.restore")}
          </Button>
          <Button type="submit">{t("style.edit.done")}</Button>
        </>
      }
    >
      <div ref={listRef} className="flex flex-col gap-6">
        <div className="relative">
          <Search
            className="pointer-events-none absolute start-[10px] top-1/2 h-[14px] w-[14px] -translate-y-1/2 text-text-3"
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
            placeholder={t("style.edit.search")}
            aria-label={t("style.edit.search")}
            className="input w-full ps-[30px] [&::-webkit-search-cancel-button]:hidden"
          />
        </div>

        <section>
          <h3 className="mb-2 text-[13px] font-semibold">
            {t("style.edit.inCategory", { category: categoryName })}{" "}
            <span className="font-normal tabular-nums text-text-3">
              {here.length}
            </span>
          </h3>
          {here.length === 0 ? (
            <p className="rounded-[var(--r-md)] border border-dashed border-line px-3 py-4 text-center text-[13px] text-text-3">
              {query.trim()
                ? t("style.edit.noMatch")
                : t("style.edit.emptyHere")}
            </p>
          ) : (
            <ul className="overflow-hidden rounded-[var(--r-md)] border border-line">
              {here.map((a) => (
                <AppRow
                  key={keyOf(a)}
                  app={a}
                  detail={detailOf(a)}
                  action={
                    category === "other" ? null : (
                      <button
                        type="button"
                        onClick={() => move(a, "other")}
                        aria-label={t("style.edit.remove", { app: a.name })}
                        title={t("style.edit.remove", { app: a.name })}
                        className="grid h-8 w-8 flex-none place-items-center rounded-[var(--r-sm)] text-text-3 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0"
                      >
                        <X className="h-4 w-4" aria-hidden="true" />
                      </button>
                    )
                  }
                />
              ))}
            </ul>
          )}
        </section>

        <section>
          <h3 className="mb-2 text-[13px] font-semibold">
            {t("style.edit.add")}
          </h3>
          <div className="flex flex-col gap-2">
            <div>
              <div className="flex gap-2">
                <input
                  id={siteId}
                  className={`input min-w-0 flex-1 ${siteError ? "invalid" : ""}`}
                  value={site}
                  onChange={(e) => {
                    setSite(e.target.value);
                    setSiteError(null);
                  }}
                  onKeyDown={(e) => {
                    if (e.key === "Enter" && !e.ctrlKey) {
                      e.preventDefault();
                      addSite();
                    }
                  }}
                  placeholder={t("style.edit.sitePlaceholder")}
                  aria-label={t("style.edit.siteLabel")}
                  aria-describedby={`${siteId}-msg`}
                  aria-invalid={!!siteError}
                  autoComplete="off"
                  spellCheck={false}
                />
                <Button
                  variant="secondary"
                  onClick={addSite}
                  disabled={!site.trim()}
                >
                  {t("style.edit.addSite")}
                </Button>
              </div>
              <p
                id={`${siteId}-msg`}
                className={`mt-1.5 text-xs ${siteError ? "text-danger" : "text-text-3"}`}
              >
                {siteError ?? t("style.edit.siteHelp")}
              </p>
            </div>
            <div className="flex flex-wrap items-center gap-3">
              <Button
                variant="secondary"
                onClick={pick}
                disabled={countdown !== null}
              >
                <Crosshair className="h-4 w-4" aria-hidden="true" />
                {t("style.edit.pick")}
              </Button>
              <span aria-live="polite" className="text-[13px] text-text-3">
                {countdown !== null
                  ? t("style.edit.pickCountdown", { count: countdown })
                  : pickMissed
                    ? t("style.edit.pickMissed")
                    : t("style.edit.pickHelp", { count: PICK_SECONDS })}
              </span>
            </div>
            {picked && (
              <ul className="overflow-hidden rounded-[var(--r-md)] border border-line">
                <AppRow
                  app={picked}
                  detail={`${t(`style.categories.${picked.category}`)} · ${detailOf(picked)}`}
                  action={
                    picked.category === category ? (
                      <span className="text-[13px] text-text-3">
                        {t("style.edit.alreadyHere")}
                      </span>
                    ) : (
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => move(picked, category)}
                      >
                        {t("style.edit.moveHere")}
                      </Button>
                    )
                  }
                />
              </ul>
            )}
          </div>
        </section>

        <section>
          <h3 className="mb-2 text-[13px] font-semibold">
            {t("style.edit.others")}{" "}
            <span className="font-normal tabular-nums text-text-3">
              {others.length}
            </span>
          </h3>
          {others.length === 0 ? (
            <p className="rounded-[var(--r-md)] border border-dashed border-line px-3 py-4 text-center text-[13px] text-text-3">
              {query.trim()
                ? t("style.edit.noMatch")
                : t("style.edit.emptyOthers")}
            </p>
          ) : (
            <ul className="overflow-hidden rounded-[var(--r-md)] border border-line">
              {others.map((a) => (
                <AppRow
                  key={keyOf(a)}
                  app={a}
                  detail={
                    a.words > 0
                      ? t("style.edit.otherDetail", {
                          category: t(`style.categories.${a.category}`),
                          count: a.words,
                          words: a.words.toLocaleString(i18n.language),
                        })
                      : t(`style.categories.${a.category}`)
                  }
                  action={
                    <Button
                      variant="secondary"
                      size="sm"
                      onClick={() => move(a, category)}
                    >
                      {t("style.edit.moveHere")}
                    </Button>
                  }
                />
              ))}
            </ul>
          )}
        </section>
      </div>
    </Sheet>
  );
};
