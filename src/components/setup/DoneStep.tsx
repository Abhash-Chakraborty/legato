import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import {
  AudioLines,
  BookA,
  ChevronRight,
  Cpu,
  Mic,
  Plug,
  Scissors,
  Type,
} from "lucide-react";
import { useSettings } from "../../hooks/useSettings";
import { useModelStore } from "../../stores/modelStore";
import { useAiStore, watchAi } from "../../stores/aiStore";
import { describeAi } from "../../lib/utils/aiStatus";
import { AiDotMark } from "../shared/AiStatusLine";
import type { StepProps } from "./SetupFlow";
import { Keys, Panel, appOf } from "./parts";
import { PRACTICE } from "./setup";
import { sceneDone, useSetupStore } from "./setupStore";

/** The end of setup: the keys to remember and how Legato is set up, not a
 *  pitch. */
export const DoneStep: React.FC<StepProps> = ({ go, setFoot, finish }) => {
  const { t } = useTranslation();
  const { settings, getSetting, updateSetting } = useSettings();
  const models = useModelStore((s) => s.models);
  const current = useModelStore((s) => s.currentModel);
  const results = useSetupStore((s) => s.results);
  const chat = useSetupStore((s) => s.chat);
  const words = useSetupStore((s) => s.words);
  const openModels = useSetupStore((s) => s.openModels);
  const ai = useAiStore((s) => s.status);
  useEffect(watchAi, []);

  useEffect(() => {
    setFoot({
      label: t("setup.done.open"),
      note: openModels ? t("setup.done.modelsNext") : "",
      onNext: () => finish(openModels ? "models" : "home"),
    });
  }, [openModels, finish, setFoot, t]);

  const activation = settings?.shortcut_activation ?? "hold_or_toggle";
  const bindings = settings?.bindings ?? {};
  const dictate = bindings.transcribe?.current_binding ?? "";
  const mic = getSetting("selected_microphone");
  const micName =
    !mic || mic === "default" || mic === "Default"
      ? t("setup.mic.default")
      : mic;
  const model = models.find((m) => m.id === current);
  const added = Object.values(words).filter((w) => w.state === "added");
  const practiced = PRACTICE.filter((s) => sceneDone({ results, chat }, s)).map(
    appOf,
  );
  const join = (list: string[]) =>
    list.length < 2
      ? list.join("")
      : t("setup.and", {
          list: list.slice(0, -1).join(", "),
          last: list[list.length - 1],
        });

  const recap = [
    {
      step: "mic" as const,
      icon: Mic,
      label: micName,
      what: t("setup.done.changeMic"),
    },
    {
      step: "model" as const,
      icon: Cpu,
      label: model?.name ?? t("setup.done.noModel"),
      what: t("setup.done.changeModel"),
    },
    {
      step: "message" as const,
      icon: AudioLines,
      label: practiced.length
        ? t("setup.done.practiced", { list: join(practiced) })
        : t("setup.done.practiceSkipped"),
      what: t("setup.done.backPractice"),
    },
    {
      step: "words" as const,
      icon: BookA,
      label: added.length
        ? t("setup.done.words", {
            list: join(added.slice(0, 2).map((w) => w.text)),
            count: added.length,
            more: Math.max(0, added.length - 2),
          })
        : t("setup.done.noWords"),
      what: t("setup.done.backWords"),
    },
  ];

  const dictateRows: [string, string][] =
    activation === "toggle"
      ? [[t("setup.done.press"), t("setup.done.pressWhat")]]
      : activation === "push_to_talk"
        ? [[t("setup.done.hold"), t("setup.done.holdWhat")]]
        : [
            [t("setup.done.hold"), t("setup.done.holdWhat")],
            [t("setup.done.tap"), t("setup.done.tapWhat")],
          ];
  // Just dictation, or everything: Notes, the Scratchpad, the bubble and
  // Insights go together. New installs start with just dictation.
  const notesOn = settings?.notes_enabled ?? true;
  const scratchpadOn = settings?.scratchpad_enabled ?? true;
  const everything =
    notesOn &&
    scratchpadOn &&
    (settings?.bubble_enabled ?? true) &&
    (settings?.insights_enabled ?? true);
  const choose = (all: boolean) => {
    if (all === everything) return;
    updateSetting("notes_enabled", all);
    updateSetting("scratchpad_enabled", all);
    updateSetting("insights_enabled", all);
    updateSetting("bubble_enabled", all);
  };

  const otherKeys = (
    [
      ["cancel", t("setup.done.keys.cancel"), true],
      ["open_scratchpad", t("setup.done.keys.scratchpad"), scratchpadOn],
      ["toggle_notes", t("setup.done.keys.notes"), notesOn],
    ] as const
  )
    .filter(([, , on]) => on)
    .map(([id, what]) => [bindings[id]?.current_binding ?? "", what] as const)
    .filter(([keys]) => keys);

  const styles = settings?.styles;
  const tone = (cat: "personal" | "email" | "code") =>
    styles ? t(`style.tones.${styles[cat].tone}.label`) : "";
  const aiLine = describeAi(ai, t);
  const calls = settings?.calls?.detect ?? false;
  const later: {
    page: "style" | "snippets" | "settings";
    sub?: string;
    icon: typeof Mic;
    title: string;
    desc: React.ReactNode;
  }[] = [
    {
      page: "style",
      icon: Type,
      title: t("setup.done.later.style"),
      desc: styles
        ? t("setup.done.later.styleDesc", {
            personal: tone("personal"),
            email: tone("email").toLowerCase(),
            code: tone("code").toLowerCase(),
          })
        : "",
    },
    {
      page: "snippets",
      icon: Scissors,
      title: t("setup.done.later.snippets"),
      desc: t("setup.done.later.snippetsDesc"),
    },
    {
      page: "settings",
      sub: "ai",
      icon: Plug,
      title:
        ai?.kind === "none" || !ai
          ? t("setup.done.later.aiNone")
          : t("setup.done.later.ai"),
      desc:
        ai?.kind === "none" || !ai ? (
          t("setup.done.later.aiNoneDesc")
        ) : (
          <span className="inline-flex items-center gap-1.5">
            <AiDotMark dot={aiLine.dot} />
            {aiLine.text}
          </span>
        ),
    },
    ...(notesOn
      ? [
          {
            page: "settings" as const,
            sub: "calls",
            icon: AudioLines,
            title: t("setup.done.later.calls"),
            desc: calls
              ? t("setup.done.later.callsOn")
              : t("setup.done.later.callsOff"),
          },
        ]
      : []),
  ];

  const autostart = getSetting("autostart_enabled") ?? false;
  const hidden = getSetting("start_hidden") ?? false;

  return (
    <Panel
      title={t("setup.done.title")}
      sub={
        dictate ? (
          <>
            {activation === "toggle"
              ? t("setup.done.subPress")
              : t("setup.done.subHold")}{" "}
            <Keys binding={dictate} className="align-middle" />{" "}
            {activation === "toggle"
              ? t("setup.done.subPressAfter")
              : t("setup.done.subHoldAfter")}
          </>
        ) : (
          t("setup.done.subNoKey")
        )
      }
      left={
        <>
          <ul className="mt-6 flex flex-col gap-1">
            {recap.map((r) => (
              <li key={r.step}>
                <button
                  type="button"
                  onClick={() => go(r.step)}
                  aria-label={`${r.label}, ${r.what}`}
                  title={r.what}
                  className="flex w-full items-center gap-2.5 rounded-[6px] px-2 py-1.5 text-start text-[13px] text-text-2 hover:bg-surface-2 hover:text-text"
                >
                  <r.icon
                    className="h-4 w-4 flex-none text-text-3"
                    aria-hidden="true"
                  />
                  <span className="truncate">{r.label}</span>
                </button>
              </li>
            ))}
          </ul>
          <fieldset className="mt-5 px-2">
            <legend className="mb-2 text-[13px] font-medium">
              {t("setup.done.how.title")}
            </legend>
            {[false, true].map((all) => (
              <label
                key={String(all)}
                className={`mb-1.5 flex cursor-pointer items-start gap-2.5 rounded-[var(--r-md)] border px-3 py-2 text-[13px] ${
                  everything === all
                    ? "border-teal-600 bg-surface-2"
                    : "border-line hover:bg-surface-2"
                }`}
              >
                <input
                  type="radio"
                  name="how-much"
                  className="mt-0.5 h-4 w-4 accent-[var(--teal-600)]"
                  checked={everything === all}
                  onChange={() => choose(all)}
                />
                <span>
                  {t(all ? "setup.done.how.all" : "setup.done.how.dictation")}
                  <span className="block text-text-3">
                    {t(
                      all
                        ? "setup.done.how.allDesc"
                        : "setup.done.how.dictationDesc",
                    )}
                  </span>
                </span>
              </label>
            ))}
            <p className="mt-1 text-[12px] text-text-3">
              {t("setup.done.how.later")}
            </p>
          </fieldset>
          <label className="mt-5 flex cursor-pointer items-start gap-2.5 px-2 text-[13px]">
            <input
              type="checkbox"
              className="mt-0.5 h-4 w-4 accent-[var(--teal-600)]"
              checked={autostart}
              onChange={(e) =>
                updateSetting("autostart_enabled", e.target.checked)
              }
            />
            <span>
              {t("setup.done.autostart")}
              <span className="block text-text-3">
                {hidden
                  ? t("setup.done.opensHidden")
                  : t("setup.done.opensWindow")}
              </span>
            </span>
          </label>
        </>
      }
      right={
        <>
          <div className="rounded-[var(--r-lg)] border border-line">
            <h2 className="border-b border-line px-4 py-2.5 text-[13px] font-medium">
              {t("setup.done.keysTitle")}
            </h2>
            {dictate &&
              dictateRows.map(([how, what]) => (
                <div
                  key={how}
                  className="grid grid-cols-[52px_auto_minmax(0,1fr)] items-center gap-3 border-b border-line px-4 py-2 text-[13px] last:border-b-0"
                >
                  <span className="text-text-3">{how}</span>
                  <Keys binding={dictate} />
                  <span>{what}</span>
                </div>
              ))}
            {otherKeys.map(([keys, what]) => (
              <div
                key={what}
                className="grid grid-cols-[52px_auto_minmax(0,1fr)] items-center gap-3 border-b border-line px-4 py-2 text-[13px] last:border-b-0"
              >
                <span />
                <Keys binding={keys} />
                <span>{what}</span>
              </div>
            ))}
          </div>
          <h2 className="mt-6 mb-2 text-[13px] font-medium">
            {t("setup.done.laterTitle")}
          </h2>
          <ul className="overflow-hidden rounded-[var(--r-lg)] border border-line">
            {later.map((row) => (
              <li
                key={row.title}
                className="border-b border-line last:border-b-0"
              >
                <button
                  type="button"
                  onClick={() => finish(row.page, row.sub)}
                  className="flex w-full items-center gap-3 px-4 py-3 text-start hover:bg-surface-2"
                >
                  <span
                    className="flex h-8 w-8 flex-none items-center justify-center rounded-[8px] bg-surface-2 text-text-2"
                    aria-hidden="true"
                  >
                    <row.icon className="h-4 w-4" />
                  </span>
                  <span className="min-w-0 flex-1">
                    <span className="block text-sm font-medium">
                      {row.title}
                    </span>
                    <span className="block truncate text-[13px] text-text-3">
                      {row.desc}
                    </span>
                  </span>
                  <ChevronRight
                    className="h-4 w-4 flex-none text-text-4"
                    aria-hidden="true"
                  />
                </button>
              </li>
            ))}
          </ul>
        </>
      }
    />
  );
};
