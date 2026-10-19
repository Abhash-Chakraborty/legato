import React, { useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { useAiStore, watchAi } from "../../stores/aiStore";
import { useNavStore } from "../../stores/navStore";
import { aiName, describeAi, type AiAction } from "../../lib/utils/aiStatus";

/** Runs Test connection and says how it went in a toast. */
export const useAiRetry = () => {
  const { t } = useTranslation();
  const test = useAiStore((s) => s.test);
  return useCallback(async () => {
    const before = useAiStore.getState().status;
    const name = before ? aiName(before, t) : t("ai.provider");
    const id = toast.loading(t("ai.checking", { name }));
    const result = await test();
    if (!result) {
      toast.dismiss(id);
      return null;
    }
    const { status } = result;
    if (status.kind === "ok")
      toast.success(t("ai.connected", { name, ms: status.latency_ms ?? 0 }), {
        id,
      });
    else toast.error(describeAi(status, t).text, { id });
    return result;
  }, [t, test]);
};

/** Runs the action a status line offers. */
export const useAiAction = () => {
  const go = useNavStore((s) => s.go);
  const retry = useAiRetry();
  return useCallback(
    (action: AiAction) => {
      if (action === "retry") void retry();
      else go("settings", "ai");
    },
    [go, retry],
  );
};

export const AiDotMark: React.FC<{ dot: "ok" | "off" | "danger" }> = ({
  dot,
}) => (
  <span
    aria-hidden="true"
    className={`inline-block h-2 w-2 flex-none rounded-full ${
      dot === "ok" ? "bg-ok" : dot === "danger" ? "bg-danger" : "bg-text-4"
    }`}
  />
);

/** "Ollama · qwen2.5:7b · local", or the problem and its one fix. The same
 *  words appear wherever the AI is used. */
export const AiStatusLine: React.FC<{
  /** False when the feature showing the line is off in Settings › AI. */
  allowed?: boolean;
  className?: string;
}> = ({ allowed = true, className = "" }) => {
  const { t } = useTranslation();
  const status = useAiStore((s) => s.status);
  const testing = useAiStore((s) => s.testing);
  const run = useAiAction();
  useEffect(watchAi, []);

  const line = describeAi(status, t, allowed);
  return (
    <span
      className={`inline-flex min-w-0 items-center gap-2 text-[13px] text-text-2 ${className}`}
    >
      <AiDotMark dot={line.dot} />
      <span className="min-w-0 truncate">{line.text}</span>
      {line.action && (
        <button
          type="button"
          className="link flex-none"
          disabled={testing}
          onClick={() => run(line.action!)}
        >
          {t(`ai.action.${line.action}`)}
        </button>
      )}
    </span>
  );
};
