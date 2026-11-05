import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { commands, type CheckScore } from "@/bindings";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";

/** Runs Cleanup at Medium on your newest hand-fixed dictations and says how
 *  many fixed words came out right, so a prompt change is measured. */
export const AiCheck: React.FC<{ grouped?: boolean }> = ({
  grouped = false,
}) => {
  const { t } = useTranslation();
  const [running, setRunning] = useState(false);
  const [score, setScore] = useState<CheckScore | null>(null);
  const [error, setError] = useState<string | null>(null);

  const run = async () => {
    setRunning(true);
    setError(null);
    const r = await commands.checkAiOnFixes().catch((e) => ({
      status: "error" as const,
      error: String(e),
    }));
    setRunning(false);
    if (r.status === "ok") setScore(r.data);
    else setError(r.error);
  };

  return (
    <SettingContainer
      title={t("settings.debug.aiCheck.title")}
      description={t("settings.debug.aiCheck.description")}
      descriptionMode="inline"
      grouped={grouped}
      layout="stacked"
    >
      <div className="flex flex-col items-start gap-2">
        <Button variant="secondary" size="md" onClick={run} disabled={running}>
          {running
            ? t("settings.debug.aiCheck.running")
            : t("settings.debug.aiCheck.button")}
        </Button>
        <p aria-live="polite" className="text-[13px] text-text-2">
          {error
            ? error
            : score === null
              ? null
              : score.cases === 0
                ? t("settings.debug.aiCheck.none")
                : t("settings.debug.aiCheck.result", {
                    answered: score.answered,
                    right: score.words_right,
                    words: score.words,
                    tooMuch: score.too_much,
                    invented: score.invented,
                  })}
        </p>
        {score && score.missed.length > 0 && (
          <p className="text-[13px] text-text-3">
            {t("settings.debug.aiCheck.missed", {
              words: score.missed.join(", "),
            })}
          </p>
        )}
      </div>
    </SettingContainer>
  );
};
