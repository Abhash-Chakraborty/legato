import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import type { Insights, InsightsRange } from "@/bindings";
import { useNavStore } from "../../../stores/navStore";
import {
  useDictionaryStore,
  watchDictionary,
} from "../../../stores/dictionaryStore";
import { SpellingList } from "../dictionary/SpellingList";
import { Button } from "../../ui/Button";
import { formatSpeech } from "../home/format";
import { busiestHour, parseDay, split, sum } from "./insights";
import { Card, Figure, ShareBar } from "./parts";
import { startsBeforeCounting } from "./UsageTab";

interface TabProps {
  data: Insights;
  range: InsightsRange;
}

const hourLabel = (hour: number, locale: string) =>
  new Date(2026, 0, 1, hour).toLocaleTimeString(locale, { hour: "numeric" });

const FixedWordsCard: React.FC<TabProps> = ({ data }) => {
  const { t, i18n } = useTranslation();
  const go = useNavStore((s) => s.go);
  const top = data.fixed_words[0]?.hits ?? 1;
  const fmt = (n: number) => n.toLocaleString(i18n.language);
  return (
    <Card
      title={t("insights.fixedWords.title")}
      aside={
        <span className="text-[13px] text-text-3">
          {t("insights.fixedWords.allTime")}
        </span>
      }
    >
      {data.fixed_words.length === 0 ? (
        <p className="py-6 text-[13px] text-text-2">
          {t("insights.fixedWords.none")}
        </p>
      ) : (
        <ul className="flex flex-col">
          {data.fixed_words.map((w) => (
            <li key={w.id} className="flex h-9 items-center gap-3 text-sm">
              <span className="flex min-w-0 flex-1 items-center gap-2">
                <span className="truncate font-medium">{w.text}</span>
                {w.learned && (
                  <span className="flex-none rounded-full bg-teal-50 px-2 py-px text-[11px] text-teal-700">
                    {t("insights.fixedWords.learned")}
                  </span>
                )}
              </span>
              <span className="hidden w-24 flex-none min-[520px]:block">
                <ShareBar share={w.hits / top} />
              </span>
              <span className="w-28 flex-none text-end text-[13px] tabular-nums text-text-2">
                {t("insights.fixedWords.times", {
                  count: w.hits,
                  times: fmt(w.hits),
                })}
              </span>
            </li>
          ))}
        </ul>
      )}
      <div className="mt-auto flex flex-wrap items-center justify-between gap-2 border-t border-line pt-3">
        <span className="text-[13px] text-text-2">
          {data.fixed_word_count > 0 &&
            t("insights.fixedWords.footer", {
              shown: data.fixed_words.length,
              count: data.fixed_word_count,
              fixes: fmt(data.fixed_word_hits),
            })}
        </span>
        <Button variant="secondary" size="sm" onClick={() => go("dictionary")}>
          {t("insights.fixedWords.open")}
        </Button>
      </div>
    </Card>
  );
};

/** Terms the model wrote more than one way in the last 30 days, whatever
 *  range the page shows. */
const SuggestedWordsCard: React.FC = () => {
  const { t } = useTranslation();
  const go = useNavStore((s) => s.go);
  const spellings = useDictionaryStore((s) => s.view?.spellings ?? null);
  useEffect(() => {
    watchDictionary();
    // New dictations since the Dictionary last loaded may add some.
    useDictionaryStore.getState().load();
  }, []);
  return (
    <Card
      title={t("insights.suggested.title")}
      className="min-[900px]:col-span-2"
      aside={
        <span className="text-[13px] text-text-3">
          {t("insights.suggested.window")}
        </span>
      }
    >
      <p className="-mt-1 mb-3 text-[13px] text-text-3">
        {t("insights.suggested.help")}
      </p>
      {spellings === null ? null : spellings.length === 0 ? (
        <div className="flex flex-wrap items-center justify-between gap-3 border-t border-line pt-3">
          <span className="text-[13px] text-text-2">
            {t("insights.suggested.none")}
          </span>
          <Button
            variant="secondary"
            size="sm"
            onClick={() => go("dictionary")}
          >
            {t("insights.fixedWords.open")}
          </Button>
        </div>
      ) : (
        <SpellingList
          spellings={spellings}
          now={Date.now() / 1000}
          className="-mx-5 -mb-5 overflow-hidden rounded-b-[var(--r-lg)] border-t border-line"
        />
      )}
    </Card>
  );
};

const FillersCard: React.FC<TabProps> = ({ data, range }) => {
  const { t, i18n } = useTranslation();
  const go = useNavStore((s) => s.go);
  const total = data.fillers.reduce((a, f) => a + f.count, 0);
  const top = data.fillers[0]?.count ?? 1;
  const shown = data.fillers.slice(0, 6);
  const since = data.counted_since;
  return (
    <Card
      title={t("insights.fillers.title")}
      aside={
        <span className="text-[13px] text-text-3">
          {t(`insights.inRange.${range}`)}
        </span>
      }
    >
      {total === 0 ? (
        <p className="py-6 text-[13px] text-text-2">
          {t("insights.fillers.none")}
        </p>
      ) : (
        <ul className="flex flex-col">
          {shown.map((f) => (
            <li key={f.text} className="flex h-9 items-center gap-3 text-sm">
              <span className="w-24 flex-none truncate">
                {t("insights.fillers.word", { word: f.text })}
              </span>
              <span className="flex-1">
                <ShareBar share={f.count / top} />
              </span>
              <span className="w-12 flex-none text-end tabular-nums text-text-2">
                {f.count.toLocaleString(i18n.language)}
              </span>
            </li>
          ))}
        </ul>
      )}
      <div className="mt-auto flex flex-wrap items-center justify-between gap-2 border-t border-line pt-3">
        <span className="text-[13px] text-text-2">
          {total > 0 &&
            t("insights.fillers.total", {
              count: total,
              words: total.toLocaleString(i18n.language),
            })}
        </span>
        <Button
          variant="secondary"
          size="sm"
          onClick={() => go("settings", "transcription")}
        >
          {t("insights.fillers.change")}
        </Button>
      </div>
      {since && startsBeforeCounting(data, range) && (
        <p className="mt-2 text-xs text-text-3">
          {t("insights.fillers.since", {
            date: parseDay(since).toLocaleDateString(i18n.language, {
              month: "long",
              day: "numeric",
            }),
          })}
        </p>
      )}
    </Card>
  );
};

const HoursCard: React.FC<TabProps> = ({ data, range }) => {
  const { t, i18n } = useTranslation();
  const max = Math.max(1, ...data.hours);
  const busiest = busiestHour(data.hours);
  return (
    <Card
      title={t("insights.hours.title")}
      aside={
        <span className="text-[13px] text-text-3">
          {t(`insights.inRange.${range}`)}
        </span>
      }
    >
      <div
        role="img"
        aria-label={
          busiest
            ? t("insights.hours.busiest", {
                from: hourLabel(busiest.hour, i18n.language),
                to: hourLabel((busiest.hour + 1) % 24, i18n.language),
                count: busiest.dictations,
              })
            : t("insights.hours.none")
        }
        className="flex h-24 items-end gap-[3px]"
      >
        {data.hours.map((n, h) => (
          <span
            key={h}
            title={t("insights.hours.bar", {
              hour: hourLabel(h, i18n.language),
              count: n,
            })}
            className={`min-w-0 flex-1 rounded-t-[2px] ${
              busiest?.hour === h ? "bg-teal-600" : "bg-teal-300"
            }`}
            style={{
              height: `${n ? Math.max(4, (n / max) * 100) : 2}%`,
              opacity: n ? 1 : 0.35,
            }}
          />
        ))}
      </div>
      <div
        aria-hidden="true"
        className="mt-1.5 grid grid-cols-4 text-[11px] text-text-3"
      >
        {[0, 6, 12, 18].map((h) => (
          <span key={h}>{hourLabel(h, i18n.language)}</span>
        ))}
      </div>
      <p className="mt-auto border-t border-line pt-3 text-[13px] text-text-2">
        {busiest
          ? t("insights.hours.busiest", {
              from: hourLabel(busiest.hour, i18n.language),
              to: hourLabel((busiest.hour + 1) % 24, i18n.language),
              count: busiest.dictations,
            })
          : t("insights.hours.none")}
      </p>
    </Card>
  );
};

const AverageCard: React.FC<TabProps> = ({ data, range }) => {
  const { t, i18n } = useTranslation();
  const now = sum(split(data.days, range, new Date()).current);
  const words = now.dictations ? Math.round(now.words / now.dictations) : 0;
  const seconds = now.dictations
    ? Math.round(now.speech_ms / now.dictations / 1000)
    : 0;
  return (
    <Card
      title={t("insights.average.title")}
      aside={
        <span className="text-[13px] text-text-3">
          {t(`insights.inRange.${range}`)}
        </span>
      }
    >
      {now.dictations === 0 ? (
        <p className="py-6 text-[13px] text-text-2">
          {t("insights.average.none")}
        </p>
      ) : (
        <div className="grid grid-cols-2 gap-4">
          <div>
            <Figure
              value={words}
              locale={i18n.language}
              unit={t("insights.average.wordsUnit")}
            />
            <p className="mt-2 text-[13px] text-text-2">
              {t("insights.average.words")}
            </p>
          </div>
          <div>
            <Figure
              value={seconds}
              locale={i18n.language}
              unit={t("insights.average.secondsUnit")}
            />
            <p className="mt-2 text-[13px] text-text-2">
              {t("insights.average.speech")}
            </p>
          </div>
        </div>
      )}
      <p className="mt-auto border-t border-line pt-3 text-[13px] text-text-2">
        {t("insights.average.footer", {
          count: now.dictations,
          dictations: now.dictations.toLocaleString(i18n.language),
          time: formatSpeech(now.speech_ms),
        })}
      </p>
    </Card>
  );
};

/** Words to add, the words Legato fixes, the fillers it removes, and when
 *  you dictate. */
export const HabitsTab: React.FC<TabProps> = ({ data, range }) => (
  <div className="grid grid-cols-1 gap-4 min-[900px]:grid-cols-2">
    <SuggestedWordsCard />
    <FixedWordsCard data={data} range={range} />
    <FillersCard data={data} range={range} />
    <HoursCard data={data} range={range} />
    <AverageCard data={data} range={range} />
  </div>
);
