import React, { useId, useState } from "react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { ChevronDown, Pin, Settings2, X } from "lucide-react";
import { commands, type DictionaryView, type LearnMode } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { Requires, SettingContainer } from "../../ui/SettingContainer";
import { Segmented } from "../../ui/Segmented";
import { AiStatusLine } from "../../shared/AiStatusLine";
import { useAiStore } from "../../../stores/aiStore";
import { MATCHING, MATCHING_THRESHOLD, matchingOf } from "./list";

/** How heard words are matched and what goes into the model's vocabulary
 *  hint, folded to one line until opened. */
export const LearningCard: React.FC<{ view: DictionaryView }> = ({ view }) => {
  const { t } = useTranslation();
  const { settings, updateSetting, isUpdating } = useSettings();
  const ai = useAiStore((s) => s.status);
  const aiConnected = !!ai && ai.kind !== "none";
  const aiOn = settings?.ai_correction_enabled ?? false;
  const [open, setOpen] = useState(false);
  const [showHint, setShowHint] = useState(false);
  const [showNever, setShowNever] = useState(false);
  const [neverInput, setNeverInput] = useState("");
  const learning = settings?.learn_from_edits ?? true;
  const mode: LearnMode = settings?.learn_mode ?? "ask";
  const bodyId = useId();

  const matching = matchingOf(settings?.word_correction_threshold ?? 0.18);
  const pinned = view.entries.filter((e) => e.pinned);
  const pinnedNames = new Set(pinned.map((e) => e.text));
  const rotating = view.hint.filter((w) => !pinnedNames.has(w));
  const free = Math.max(0, view.hint_limit - view.hint.length);

  const summary = [
    learning
      ? t(`dictionary.learning.summary.edits.${mode}`)
      : t("dictionary.learning.summary.edits.off"),
    t(`dictionary.learning.summary.${matching}`),
    t("dictionary.learning.summary.pinned", {
      pinned: pinned.length,
      limit: view.pin_limit,
    }),
    t("dictionary.learning.summary.hint", {
      used: view.hint.length,
      limit: view.hint_limit,
    }),
    ...(aiOn && aiConnected ? [t("dictionary.learning.summary.ai")] : []),
  ].join(" · ");

  return (
    <div className="mb-6 rounded-[var(--r-lg)] border border-line bg-surface">
      <button
        type="button"
        aria-expanded={open}
        aria-controls={bodyId}
        onClick={() => setOpen(!open)}
        className="flex w-full items-center gap-3 rounded-[var(--r-lg)] px-4 py-3 text-start transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0"
      >
        <span className="grid h-7 w-7 flex-none place-items-center rounded-[var(--r-sm)] bg-surface-2 text-text-2">
          <Settings2 className="h-4 w-4 stroke-[1.75]" aria-hidden="true" />
        </span>
        <span className="min-w-0 flex-1">
          <span className="me-2 text-sm font-medium">
            {t("dictionary.learning.title")}
          </span>
          <span className="text-[13px] text-text-3">{summary}</span>
        </span>
        <ChevronDown
          aria-hidden="true"
          className={`h-4 w-4 flex-none text-text-3 transition-transform duration-[var(--dur-2)] ${open ? "rotate-180" : ""}`}
        />
      </button>
      {open && (
        <div id={bodyId} className="border-t border-line px-4">
          <SettingContainer
            title={t("dictionary.learning.edits.title")}
            description={t("dictionary.learning.edits.help")}
          >
            <button
              type="button"
              role="switch"
              aria-checked={learning}
              aria-label={t("dictionary.learning.edits.title")}
              className="toggle"
              onClick={() => {
                if (!isUpdating("learn_from_edits"))
                  updateSetting("learn_from_edits", !learning);
              }}
            />
          </SettingContainer>
          <div className="border-t border-line">
            <Requires
              met={learning}
              reason={t("dictionary.learning.edits.needsOn")}
            >
              <SettingContainer
                title={t("dictionary.learning.mode.title")}
                description={t(`dictionary.learning.mode.help.${mode}`)}
              >
                <Segmented
                  label={t("dictionary.learning.mode.title")}
                  value={mode}
                  onChange={(value) => updateSetting("learn_mode", value)}
                  options={[
                    { value: "ask", label: t("dictionary.learning.mode.ask") },
                    {
                      value: "auto",
                      label: t("dictionary.learning.mode.auto"),
                    },
                  ]}
                />
              </SettingContainer>
            </Requires>
          </div>
          <div className="border-t border-line">
            <SettingContainer
              title={t("dictionary.learning.never.title")}
              description={
                view.never.length
                  ? t(
                      view.never.length > 4
                        ? "dictionary.learning.never.someMore"
                        : "dictionary.learning.never.some",
                      {
                        words: view.never
                          .slice(0, 4)
                          .map((w) => t("dictionary.row.quoted", { text: w }))
                          .join(", "),
                        more: view.never.length - 4,
                      },
                    )
                  : t("dictionary.learning.never.none")
              }
            >
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setShowNever(true)}
              >
                {t("dictionary.learning.never.edit")}
              </Button>
            </SettingContainer>
          </div>
          <div className="border-t border-line">
            <SettingContainer
              title={t("dictionary.learning.matching.title")}
              description={t(`dictionary.learning.matching.help.${matching}`)}
            >
              <div
                role="radiogroup"
                aria-label={t("dictionary.learning.matching.title")}
                className="inline-flex rounded-[var(--r-sm)] border border-line bg-surface-2 p-0.5"
              >
                {MATCHING.map((m) => (
                  <button
                    key={m}
                    type="button"
                    role="radio"
                    aria-checked={matching === m}
                    onClick={() =>
                      updateSetting(
                        "word_correction_threshold",
                        MATCHING_THRESHOLD[m],
                      )
                    }
                    className={`h-7 rounded-[calc(var(--r-sm)-2px)] px-3 text-[13px] transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                      matching === m
                        ? "bg-surface font-medium text-text shadow-[var(--shadow-1)]"
                        : "text-text-2 hover:text-text"
                    }`}
                  >
                    {t(`dictionary.learning.matching.${m}`)}
                  </button>
                ))}
              </div>
            </SettingContainer>
          </div>
          <div className="border-t border-line">
            <SettingContainer
              title={t("dictionary.learning.hint.title")}
              description={t("dictionary.learning.hint.help", {
                limit: view.hint_limit,
                pinned: pinned.length,
              })}
            >
              <Button
                variant="secondary"
                size="sm"
                onClick={() => setShowHint(true)}
              >
                {t("dictionary.learning.hint.show")}
              </Button>
            </SettingContainer>
          </div>
          <div className="border-t border-line">
            <SettingContainer
              title={t("dictionary.learning.ai.title")}
              description={
                <>
                  <span className="block">
                    {t("dictionary.learning.ai.help")}
                  </span>
                  <AiStatusLine className="mt-1" />
                </>
              }
            >
              <button
                type="button"
                role="switch"
                aria-checked={aiOn && aiConnected}
                aria-label={t("dictionary.learning.ai.title")}
                className="toggle"
                disabled={!aiConnected}
                onClick={() => {
                  if (!isUpdating("ai_correction_enabled"))
                    updateSetting("ai_correction_enabled", !aiOn);
                }}
              />
            </SettingContainer>
          </div>
        </div>
      )}
      <Dialog
        open={showNever}
        onOpenChange={setShowNever}
        title={t("dictionary.learning.never.dialogTitle")}
        description={t("dictionary.learning.never.dialogDescription")}
        closeLabel={t("dictionary.learning.hint.close")}
      >
        <form
          className="mb-3 flex gap-2"
          onSubmit={async (e) => {
            e.preventDefault();
            const word = neverInput.trim();
            if (!word) return;
            const result = await commands.addNeverLearnWord(word);
            if (result.status === "ok") setNeverInput("");
          }}
        >
          <input
            value={neverInput}
            onChange={(e) => setNeverInput(e.target.value)}
            placeholder={t("dictionary.learning.never.placeholder")}
            aria-label={t("dictionary.learning.never.placeholder")}
            className="input compact min-w-0 flex-1"
          />
          <Button type="submit" size="sm" disabled={!neverInput.trim()}>
            {t("dictionary.learning.never.add")}
          </Button>
        </form>
        {view.never.length === 0 ? (
          <p className="text-sm text-text-3">
            {t("dictionary.learning.never.empty")}
          </p>
        ) : (
          <ul className="flex flex-wrap gap-1.5">
            {view.never.map((word) => (
              <li
                key={word}
                className="inline-flex h-[26px] items-center gap-1 rounded-[var(--r-xs)] bg-surface-2 ps-2.5 pe-1 text-[13px]"
              >
                {word}
                <button
                  type="button"
                  onClick={async () => {
                    await commands.removeNeverLearnWord(word);
                    toast(t("dictionary.learning.never.removed", { word }), {
                      action: {
                        label: t("dictionary.toast.undo"),
                        onClick: () => {
                          commands.addNeverLearnWord(word).catch(() => {});
                        },
                      },
                    });
                  }}
                  aria-label={t("dictionary.learning.never.remove", { word })}
                  title={t("dictionary.learning.never.remove", { word })}
                  className="grid h-5 w-5 place-items-center rounded text-text-3 hover:bg-surface-4 hover:text-text"
                >
                  <X className="h-3 w-3" aria-hidden="true" />
                </button>
              </li>
            ))}
          </ul>
        )}
      </Dialog>
      <Dialog
        open={showHint}
        onOpenChange={setShowHint}
        title={t("dictionary.learning.hint.dialogTitle")}
        description={t("dictionary.learning.hint.dialogDescription")}
        closeLabel={t("dictionary.learning.hint.close")}
      >
        {view.hint.length === 0 ? (
          <p className="text-sm text-text-3">
            {t("dictionary.learning.hint.empty")}
          </p>
        ) : (
          <div className="flex flex-wrap gap-1.5">
            {view.hint.map((word) =>
              pinnedNames.has(word) ? (
                <span
                  key={word}
                  className="inline-flex h-[26px] items-center gap-1 rounded-[var(--r-xs)] bg-teal-50 px-2.5 text-[13px] font-medium text-teal-700"
                >
                  <Pin className="h-3 w-3" aria-hidden="true" />
                  {word}
                </span>
              ) : (
                <span
                  key={word}
                  className="inline-flex h-[26px] items-center rounded-[var(--r-xs)] bg-surface-2 px-2.5 text-[13px]"
                >
                  {word}
                </span>
              ),
            )}
          </div>
        )}
        <p className="mt-3 text-[13px] text-text-3">
          {t("dictionary.learning.hint.counts", {
            pinned: pinned.length,
            recent: rotating.length,
            free,
          })}
        </p>
      </Dialog>
    </div>
  );
};
