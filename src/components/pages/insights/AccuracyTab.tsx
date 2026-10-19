import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { Insights, InsightsModel, InsightsRange } from "@/bindings";
import { useModelStore } from "../../../stores/modelStore";
import { useNavStore } from "../../../stores/navStore";
import { Button } from "../../ui/Button";
import { parseDay } from "./insights";

interface TabProps {
  data: Insights;
  range: InsightsRange;
}

/** Fixes per 1,000 words, once a model has written enough to say. */
const fixRate = (m: InsightsModel): number | null =>
  m.all_words >= 200 ? (m.fixes / m.all_words) * 1000 : null;

const pasteMs = (m: InsightsModel): number | null =>
  m.pastes > 0 ? Math.round(m.paste_ms / m.pastes) : null;

/** Each speech model's words, time to paste and fixes, side by side. */
export const AccuracyTab: React.FC<TabProps> = ({ data }) => {
  const { t, i18n } = useTranslation();
  const go = useNavStore((s) => s.go);
  const models = useModelStore((s) => s.models);
  const current = useModelStore((s) => s.currentModel);
  const initialized = useModelStore((s) => s.initialized);
  const initialize = useModelStore((s) => s.initialize);
  const selectModel = useModelStore((s) => s.selectModel);
  useEffect(() => {
    if (!initialized) initialize();
  }, [initialized, initialize]);

  const info = (id: string) => models.find((m) => m.id === id);
  const fmt = (n: number) => n.toLocaleString(i18n.language);
  const lastUsed = (day: string) =>
    parseDay(day).toLocaleDateString(i18n.language, {
      month: "short",
      day: "numeric",
    });
  const languages = (id: string) => {
    const langs = info(id)?.supported_languages ?? [];
    if (langs.length === 0) return "—";
    if (langs.length === 1 && langs[0] === "en")
      return t("insights.models.english");
    return t("insights.models.languages", { count: langs.length });
  };

  const head =
    "px-3 py-2.5 text-start text-xs font-medium text-text-3 whitespace-nowrap";
  const cell = "px-3 py-3 tabular-nums whitespace-nowrap";

  return (
    <div className="flex flex-col gap-4">
      <div className="overflow-x-auto rounded-[var(--r-lg)] border border-line bg-surface">
        <table className="w-full min-w-[760px] border-collapse text-sm">
          <caption className="sr-only">{t("insights.models.caption")}</caption>
          <thead className="border-b border-line">
            <tr>
              <th scope="col" className={`${head} ps-5`}>
                {t("insights.models.model")}
              </th>
              <th scope="col" className={`${head} text-end`}>
                {t("insights.models.words")}
              </th>
              <th scope="col" className={`${head} text-end`}>
                {t("insights.models.dictations")}
              </th>
              <th scope="col" className={`${head} text-end`}>
                {t("insights.models.paste")}
              </th>
              <th scope="col" className={`${head} text-end`}>
                {t("insights.models.fixes")}
              </th>
              <th scope="col" className={head}>
                {t("insights.models.languagesHead")}
              </th>
              <th scope="col" className={head}>
                {t("insights.models.lastUsed")}
              </th>
              <th scope="col" className={`${head} pe-5`}>
                <span className="sr-only">{t("insights.models.actions")}</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {data.models.map((m) => {
              const model = info(m.model);
              const ms = pasteMs(m);
              const rate = fixRate(m);
              const inUse = m.model === current;
              return (
                <tr
                  key={m.model}
                  className="border-b border-line last:border-b-0"
                >
                  <th
                    scope="row"
                    className="px-3 py-3 ps-5 text-start font-medium"
                  >
                    <span className="inline-flex items-center gap-2">
                      {model?.name ?? m.model}
                      {inUse && (
                        <span className="rounded-full bg-teal-50 px-2 py-px text-[11px] font-normal text-teal-700">
                          {t("insights.models.inUse")}
                        </span>
                      )}
                      {!model && (
                        <span className="rounded-full bg-surface-2 px-2 py-px text-[11px] font-normal text-text-3">
                          {t("insights.models.removed")}
                        </span>
                      )}
                    </span>
                  </th>
                  <td className={`${cell} text-end`}>
                    {m.dictations ? fmt(m.words) : "—"}
                  </td>
                  <td className={`${cell} text-end`}>
                    {m.dictations ? fmt(m.dictations) : "—"}
                  </td>
                  <td className={`${cell} text-end`}>
                    {ms === null
                      ? "—"
                      : t("insights.models.ms", { ms: fmt(ms) })}
                  </td>
                  <td className={`${cell} text-end`}>
                    {rate === null
                      ? "—"
                      : rate.toLocaleString(i18n.language, {
                          maximumFractionDigits: 1,
                          minimumFractionDigits: 1,
                        })}
                  </td>
                  <td className={`${cell} text-text-2`}>
                    {languages(m.model)}
                  </td>
                  <td className={`${cell} text-text-2`}>
                    {lastUsed(m.last_day)}
                  </td>
                  <td className={`${cell} pe-5 text-end`}>
                    {model?.is_downloaded && !inUse && (
                      <Button
                        variant="secondary"
                        size="sm"
                        onClick={() => selectModel(m.model)}
                      >
                        {t("insights.models.use")}
                      </Button>
                    )}
                  </td>
                </tr>
              );
            })}
          </tbody>
        </table>
      </div>
      <div className="flex flex-wrap items-start justify-between gap-4">
        <p className="max-w-[720px] text-xs leading-relaxed text-text-3">
          {t("insights.models.note")}
        </p>
        <Button variant="secondary" size="sm" onClick={() => go("models")}>
          {t("insights.models.open")}
        </Button>
      </div>
    </div>
  );
};
