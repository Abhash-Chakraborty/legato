import React, { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Check, ChevronLeft } from "lucide-react";
import { commands, events } from "@/bindings";
import type { Page } from "../../stores/navStore";
import LegatoGlyph from "../icons/LegatoGlyph";
import { Button } from "../ui/Button";
import {
  GROUPS,
  PRACTICE,
  STEPS,
  groupOf,
  isScene,
  practiceMode,
  type Group,
  type Step,
} from "./setup";
import { sceneDone, useSetupStore } from "./setupStore";
import { MicStep } from "./MicStep";
import { ModelStep } from "./ModelStep";
import { PracticeStep } from "./PracticeStep";
import { WordsStep } from "./WordsStep";
import { DoneStep } from "./DoneStep";

/** The footer as the step on screen wants it. */
export interface Foot {
  back?: boolean;
  label?: string;
  kind?: "primary" | "secondary";
  disabled?: boolean;
  /** Text or a small button next to Continue. */
  note?: React.ReactNode;
  onNext?: () => void;
}

export interface StepProps {
  go: (step: Step) => void;
  setFoot: (foot: Foot) => void;
  /** Ends setup and opens a page. */
  finish: (page: Page, sub?: string) => void;
}

interface SetupFlowProps {
  initialStep: Step;
  /** Setup is over (finished or skipped); open this page. */
  onDone: (page: Page, sub?: string) => void;
}

/** First-run setup: the microphone, a speech model, three practice scenes
 *  and the words the practice got wrong. */
export const SetupFlow: React.FC<SetupFlowProps> = ({
  initialStep,
  onDone,
}) => {
  const { t } = useTranslation();
  const [step, setStep] = useState<Step>(initialStep);
  const [dir, setDir] = useState(0);
  const [foot, setFootState] = useState<Foot>({});
  const results = useSetupStore((s) => s.results);
  const chat = useSetupStore((s) => s.chat);
  const footRef = useRef<Foot>({});
  const stageRef = useRef<HTMLDivElement>(null);
  const hinted = useRef<Step | null>(null);
  const shown = useRef<Step | null>(null);

  const setFoot = useCallback((f: Foot) => {
    footRef.current = f;
    setFootState(f);
  }, []);

  const go = useCallback(
    (next: Step) => {
      setDir(Math.sign(STEPS.indexOf(next) - STEPS.indexOf(step)) || 1);
      setFoot({});
      setStep(next);
    },
    [step, setFoot],
  );

  // The shortcut works with the window in the background, so shortcuts load
  // now rather than after setup; the step decides where a dictation goes.
  useEffect(() => {
    commands.initializeShortcuts().catch(() => {});
    commands.initializeEnigo().catch(() => {});
    return () => {
      commands.setPracticeMode("off");
    };
  }, []);

  useEffect(() => {
    commands.setPracticeMode(practiceMode(step));
    commands.changeSetupStepSetting(step);
    hinted.current = null;
    // A new step takes focus at its title; the first one doesn't.
    if (shown.current && shown.current !== step)
      stageRef.current
        ?.querySelector<HTMLElement>("h1")
        ?.focus({ preventScroll: true });
    shown.current = step;
    stageRef.current?.closest("[data-setup-scroll]")?.scrollTo({ top: 0 });
  }, [step]);

  // Off the practice the shortcut only says where it will work.
  useEffect(() => {
    const off = events.practiceHint.listen(() => {
      if (hinted.current === step || isScene(step)) return;
      hinted.current = step;
      toast.info(t(`setup.hint.${step}`), { duration: 2600 });
    });
    return () => {
      off.then((fn) => fn());
    };
  }, [step, t]);

  const end = useCallback(
    (page: Page, sub?: string) => {
      commands.setPracticeMode("off");
      commands.changeSetupStepSetting("");
      useSetupStore.getState().reset();
      onDone(page, sub);
    },
    [onDone],
  );

  const skip = () => {
    const at = step;
    commands.setPracticeMode("off");
    commands.changeSetupStepSetting("");
    onDone("home");
    toast(t("setup.skipped"), {
      action: {
        label: t("setup.backToSetup"),
        onClick: () => {
          commands.changeSetupStepSetting(at);
          window.dispatchEvent(
            new CustomEvent("legato:setup", { detail: { step: at } }),
          );
        },
      },
    });
  };

  // Enter continues, unless focus is on something that takes Enter itself.
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Enter" || e.repeat || e.defaultPrevented) return;
      if (e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return;
      const el = e.target as HTMLElement | null;
      if (
        el?.closest(
          "button, a, input, textarea, select, [role=radio], [role=checkbox], [role=menu], [role=dialog]",
        )
      )
        return;
      const f = footRef.current;
      if (f.disabled || !f.onNext) return;
      e.preventDefault();
      f.onNext();
    };
    document.addEventListener("keydown", onKey);
    return () => document.removeEventListener("keydown", onKey);
  }, []);

  const group = groupOf(step);
  const at = group ? GROUPS.indexOf(group) : GROUPS.length;
  const groupStep = (g: Group): Step => (g === "practice" ? "message" : g);
  const stepIndex = STEPS.indexOf(step);
  const props: StepProps = { go, setFoot, finish: end };

  return (
    <div
      data-setup-scroll
      className="h-full overflow-y-auto bg-surface [scrollbar-gutter:stable]"
    >
      <div className="mx-auto flex min-h-full w-full max-w-[1040px] flex-col px-8 pt-6 pb-6 min-[1180px]:px-12">
        <header className="flex items-center gap-6">
          <span className="flex flex-none items-center gap-2 text-[15px] font-semibold">
            <span className="flex h-7 w-7 items-center justify-center rounded-[8px] bg-teal-fill text-white">
              <LegatoGlyph width={16} height={16} />
            </span>
            {t("setup.brand")}
          </span>
          <nav
            aria-label={t("setup.steps")}
            className="flex min-w-0 flex-1 items-center gap-3 text-[13px]"
          >
            {GROUPS.map((g, k) => {
              const past = k < at;
              const current = k === at;
              const label = t(`setup.group.${g}`);
              const dots =
                g === "practice" ? (
                  <span className="flex gap-[3px]" aria-hidden="true">
                    {PRACTICE.map((s) => (
                      <i
                        key={s}
                        className={`block h-[5px] w-[5px] rounded-full ${
                          step === s
                            ? "bg-teal-600"
                            : sceneDone({ results, chat }, s)
                              ? "bg-teal-300"
                              : "bg-line-strong"
                        }`}
                      />
                    ))}
                  </span>
                ) : null;
              return (
                <React.Fragment key={g}>
                  {k > 0 && (
                    <span
                      className="h-px w-4 flex-none bg-line-strong"
                      aria-hidden="true"
                    />
                  )}
                  {past ? (
                    <button
                      type="button"
                      onClick={() => go(groupStep(g))}
                      className="flex items-center gap-1.5 rounded-[6px] px-1 py-1 text-text-3 hover:text-text"
                    >
                      <Check className="h-3.5 w-3.5" aria-hidden="true" />
                      {label}
                      {dots}
                      <span className="sr-only">{t("setup.finished")}</span>
                    </button>
                  ) : (
                    <span
                      aria-current={current ? "step" : undefined}
                      className={`relative flex items-center gap-1.5 px-1 py-1 ${
                        current
                          ? "font-medium text-text after:absolute after:inset-x-0 after:-bottom-1.5 after:h-0.5 after:rounded-full after:bg-teal-600"
                          : "text-text-4"
                      }`}
                    >
                      {label}
                      {dots}
                      {current && g === "practice" && (
                        <span className="sr-only">
                          {t("setup.sceneOf", {
                            n: PRACTICE.indexOf(step as never) + 1,
                          })}
                        </span>
                      )}
                    </span>
                  )}
                </React.Fragment>
              );
            })}
          </nav>
          {step !== "done" && (
            <button
              type="button"
              onClick={skip}
              className="flex-none rounded-[6px] px-2 py-1 text-[13px] text-text-2 hover:bg-surface-2 hover:text-text"
            >
              {t("setup.skip")}
            </button>
          )}
        </header>

        <div
          ref={stageRef}
          key={step}
          className="setup-enter flex-1 pt-10 pb-8"
          style={{ "--setup-dir": dir } as React.CSSProperties}
        >
          {step === "mic" && <MicStep {...props} />}
          {step === "model" && <ModelStep {...props} />}
          {isScene(step) && <PracticeStep key={step} scene={step} {...props} />}
          {step === "words" && <WordsStep {...props} />}
          {step === "done" && <DoneStep {...props} />}
        </div>

        <footer className="sticky bottom-0 flex items-center gap-3 border-t border-line bg-surface py-4">
          {foot.back !== false && stepIndex > 0 ? (
            <Button
              variant="ghost"
              size="md"
              onClick={() => go(STEPS[stepIndex - 1])}
              className="-ms-2"
            >
              <ChevronLeft className="h-4 w-4" aria-hidden="true" />
              {t("common.back")}
            </Button>
          ) : (
            <span />
          )}
          <span className="ms-auto text-[13px] text-text-3">{foot.note}</span>
          <Button
            variant={foot.kind ?? "primary"}
            size="lg"
            disabled={foot.disabled || !foot.onNext}
            onClick={() => foot.onNext?.()}
            className="min-w-[120px]"
          >
            {foot.label ?? t("setup.continue")}
          </Button>
        </footer>
      </div>
    </div>
  );
};
