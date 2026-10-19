import React, { useEffect, useId, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { ChevronRight, X } from "lucide-react";
import {
  commands,
  type CleanupLevel,
  type CleanupRules,
  type HistoryEntry,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useAiStore, watchAi } from "../../../stores/aiStore";
import { useNavStore } from "../../../stores/navStore";
import { AiStatusLine } from "../../shared/AiStatusLine";
import { Segmented } from "../../ui/Segmented";
import { ToggleSwitch } from "../../ui/ToggleSwitch";
import { AppIcon } from "../home/AppIcon";
import { relativeTime } from "../dictionary/list";
import { heardParts } from "./heard";
import { asCategory } from "./model";

const LEVELS: CleanupLevel[] = ["none", "light", "medium"];
const RULES: (keyof CleanupRules)[] = [
  "repeats",
  "numbers",
  "punctuation",
  "corrections",
  "lists",
];

/** What a rule is set to; every rule starts on. */
const ruleOn = (rules: CleanupRules | undefined, rule: keyof CleanupRules) =>
  rules?.[rule] ?? true;

/** The fillers to drop, as chips; editing them makes a custom list. */
const FillerChips: React.FC<{ disabled: boolean }> = ({ disabled }) => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const [builtIn, setBuiltIn] = useState<string[]>([]);
  const [input, setInput] = useState("");
  const inputRef = useRef<HTMLInputElement>(null);
  const custom = settings?.custom_filler_words ?? null;
  const words = custom ?? builtIn;

  useEffect(() => {
    commands
      .getDefaultFillerWords()
      .then(setBuiltIn)
      .catch(() => {});
  }, []);

  const save = (next: string[]) => updateSetting("custom_filler_words", next);
  const add = () => {
    const word = input.trim().replace(/,$/, "").trim().toLowerCase();
    setInput("");
    if (word && !words.includes(word)) save([...words, word]);
  };

  return (
    <div className="pb-3">
      <div
        className={`flex min-h-[38px] cursor-text flex-wrap items-center gap-1.5 rounded-[var(--r-md)] border border-line-strong bg-surface px-2 py-[5px] transition-[border-color,box-shadow] duration-[var(--dur-1)] focus-within:border-teal-600 focus-within:shadow-[0_0_0_3px_var(--teal-50)] ${disabled ? "pointer-events-none opacity-50" : ""}`}
        onClick={() => inputRef.current?.focus()}
      >
        {words.map((w) => (
          <span
            key={w}
            className="inline-flex h-6 items-center gap-1 rounded-[var(--r-xs)] bg-surface-3 ps-2 pe-1 text-[13px]"
          >
            {w}
            <button
              type="button"
              disabled={disabled}
              onClick={(e) => {
                e.stopPropagation();
                save(words.filter((x) => x !== w));
              }}
              aria-label={t("style.rules.fillers.remove", { word: w })}
              title={t("style.rules.fillers.remove", { word: w })}
              className="grid h-[18px] w-[18px] place-items-center rounded text-text-3 hover:bg-surface-4 hover:text-text"
            >
              <X className="h-3 w-3" aria-hidden="true" />
            </button>
          </span>
        ))}
        <input
          ref={inputRef}
          value={input}
          disabled={disabled}
          onChange={(e) => setInput(e.target.value)}
          onKeyDown={(e) => {
            if ((e.key === "Enter" && !e.ctrlKey) || e.key === ",") {
              e.preventDefault();
              add();
            } else if (e.key === "Backspace" && !input && words.length > 0) {
              save(words.slice(0, -1));
            }
          }}
          onBlur={add}
          aria-label={t("style.rules.fillers.add")}
          placeholder={t("style.rules.fillers.add")}
          className="h-[26px] min-w-[140px] flex-1 border-0 bg-transparent text-sm text-text outline-none placeholder:text-text-4"
          autoComplete="off"
          spellCheck={false}
        />
      </div>
      <p className="mt-1.5 text-[13px] text-text-3">
        {custom ? (
          <>
            {t("style.rules.fillers.custom")}{" "}
            <button
              type="button"
              disabled={disabled}
              onClick={() => updateSetting("custom_filler_words", null)}
              className="link"
            >
              {t("style.rules.fillers.reset")}
            </button>
          </>
        ) : (
          t("style.rules.fillers.builtIn")
        )}
      </p>
    </div>
  );
};

/** The last dictation, heard and as Legato would paste it now. */
const Example: React.FC<{ entry: HistoryEntry; level: CleanupLevel }> = ({
  entry,
  level,
}) => {
  const { t, i18n } = useTranslation();
  const { settings } = useSettings();
  const heard = entry.heard_text ?? "";
  const [pasted, setPasted] = useState<string | null>(null);
  // Re-run when anything the preview depends on changes.
  const key = JSON.stringify([
    settings?.cleanup_rules,
    settings?.styles,
    settings?.custom_filler_words,
    settings?.filler_word_removal_enabled,
  ]);

  useEffect(() => {
    let live = true;
    commands
      .previewStyle(heard, asCategory(entry.category), null, level)
      .then((text) => live && setPasted(text))
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [heard, entry.category, level, key]);

  const parts = useMemo(
    () => (pasted === null ? [] : heardParts(heard, pasted)),
    [heard, pasted],
  );
  const now = Math.floor(Date.now() / 1000);

  return (
    <div className="rounded-[var(--r-md)] bg-surface-2 px-4 py-3 text-sm">
      <div className="mb-2 flex items-center gap-1.5 text-xs text-text-3">
        <AppIcon path={entry.app_path} className="h-3.5 w-3.5" />
        {entry.app_name
          ? t("style.cleanup.exampleFrom", {
              app: entry.app_site ?? entry.app_name,
              when: relativeTime(entry.timestamp, now, i18n.language),
            })
          : t("style.cleanup.exampleLast", {
              when: relativeTime(entry.timestamp, now, i18n.language),
            })}
      </div>
      <dl className="grid grid-cols-[64px_1fr] gap-x-3 gap-y-1.5 leading-relaxed">
        <dt className="text-[13px] text-text-3">{t("style.cleanup.heard")}</dt>
        <dd className="min-w-0 break-words">
          {parts.length === 0
            ? heard
            : parts.map((p, i) =>
                p.removed ? (
                  <React.Fragment key={i}>
                    <del className="text-text-3 decoration-danger/70">
                      {p.text.trimEnd()}
                    </del>
                    {p.text.slice(p.text.trimEnd().length)}
                  </React.Fragment>
                ) : (
                  <span key={i}>{p.text}</span>
                ),
              )}
        </dd>
        <dt className="text-[13px] text-text-3">{t("style.cleanup.pasted")}</dt>
        <dd className="min-w-0 whitespace-pre-wrap break-words">
          {pasted ?? ""}
        </dd>
      </dl>
    </div>
  );
};

/** Cleanup for every dictation: the level, the rules behind Light, and the
 *  last dictation as an example. */
export const CleanupCard: React.FC<{ latest: HistoryEntry | null }> = ({
  latest,
}) => {
  const { t } = useTranslation();
  const { settings, updateSetting } = useSettings();
  const go = useNavStore((s) => s.go);
  const ai = useAiStore((s) => s.status);
  const [open, setOpen] = useState(false);
  const bodyId = useId();
  useEffect(watchAi, []);

  const level = settings?.cleanup_level ?? "light";
  const rules = settings?.cleanup_rules;
  const fillers = settings?.filler_word_removal_enabled ?? true;
  const aiReady = !!ai && ai.kind !== "none";
  const off = level === "none";

  const onCount = RULES.filter((r) => ruleOn(rules, r)).length + +fillers;
  const onNames = [
    ...(fillers ? [t("style.rules.names.fillers")] : []),
    ...RULES.filter((r) => ruleOn(rules, r)).map((r) =>
      t(`style.rules.names.${r}`),
    ),
  ].join(", ");

  const setRule = (rule: keyof CleanupRules, on: boolean) =>
    updateSetting("cleanup_rules", {
      ...Object.fromEntries(RULES.map((r) => [r, ruleOn(rules, r)])),
      [rule]: on,
    });

  return (
    <section className="mb-10 overflow-hidden rounded-[var(--r-lg)] border border-line bg-surface">
      <div className="px-5 pt-4 pb-4">
        <div className="flex flex-wrap items-start justify-between gap-x-6 gap-y-3">
          <div className="min-w-0 flex-[1_1_320px]">
            <h2 className="text-[15px] font-semibold">
              {t("style.cleanup.title")}
            </h2>
            <p className="mt-0.5 text-sm text-text-2">
              {t(`style.cleanup.levels.${level}.help`)}
            </p>
          </div>
          <div className="flex flex-col items-end gap-1.5">
            <Segmented
              label={t("style.cleanup.title")}
              value={level}
              onChange={(v) => updateSetting("cleanup_level", v)}
              options={LEVELS.map((l) => ({
                value: l,
                label: t(`style.cleanup.levels.${l}.label`),
                disabled: l === "medium" && !aiReady && level !== "medium",
                title:
                  l === "medium" && !aiReady
                    ? t("style.cleanup.mediumNeedsAi")
                    : undefined,
              }))}
            />
            {aiReady ? (
              <span className="inline-flex max-w-full items-center gap-1.5 text-xs text-text-3">
                {t("style.cleanup.mediumUses")}
                <AiStatusLine className="!text-xs" />
              </span>
            ) : (
              <span className="text-xs text-text-3">
                {t("style.cleanup.mediumNeedsAi")}{" "}
                <button
                  type="button"
                  className="link"
                  onClick={() => go("settings", "ai")}
                >
                  {t("style.cleanup.setUpAi")}
                </button>
              </span>
            )}
          </div>
        </div>
        {latest?.heard_text && (
          <div className="mt-4">
            <Example entry={latest} level={level} />
            {level === "medium" && (
              <p className="mt-1.5 text-xs text-text-3">
                {t("style.cleanup.mediumPreview")}
              </p>
            )}
          </div>
        )}
      </div>
      <button
        type="button"
        aria-expanded={open}
        aria-controls={bodyId}
        onClick={() => setOpen(!open)}
        className="flex w-full items-center gap-2 border-t border-line px-5 py-3 text-start transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0"
      >
        <ChevronRight
          aria-hidden="true"
          className={`h-4 w-4 flex-none text-text-3 transition-transform duration-[var(--dur-2)] ${open ? "rotate-90" : ""}`}
        />
        <span className="text-sm font-medium">{t("style.rules.title")}</span>
        <span className="min-w-0 truncate text-[13px] text-text-3">
          {off
            ? t("style.rules.offWithCleanup")
            : t("style.rules.summary", {
                on: onCount,
                total: RULES.length + 1,
                names: onNames || t("style.rules.none"),
              })}
        </span>
      </button>
      {open && (
        <div id={bodyId} className="border-t border-line px-5 ps-11">
          <ToggleSwitch
            checked={fillers}
            onChange={(v) => updateSetting("filler_word_removal_enabled", v)}
            disabled={off}
            label={t("style.rules.fillers.title")}
            description={
              off
                ? t("style.rules.offWithCleanup")
                : t("style.rules.fillers.help")
            }
            grouped
          />
          {fillers && <FillerChips disabled={off} />}
          {RULES.map((rule) => (
            <div key={rule} className="border-t border-line">
              <ToggleSwitch
                checked={ruleOn(rules, rule)}
                onChange={(v) => setRule(rule, v)}
                disabled={off}
                label={t(`style.rules.${rule}.title`)}
                description={
                  off
                    ? t("style.rules.offWithCleanup")
                    : t(`style.rules.${rule}.help`)
                }
                grouped
              />
            </div>
          ))}
        </div>
      )}
    </section>
  );
};
