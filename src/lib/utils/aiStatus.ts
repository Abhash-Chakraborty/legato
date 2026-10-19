import type { TFunction } from "i18next";
import type { AiStatus } from "@/bindings";

export type AiDot = "ok" | "off" | "danger";
export type AiAction = "setup" | "retry" | "fix" | "change";

export interface AiLine {
  dot: AiDot;
  text: string;
  action: AiAction | null;
}

/** The provider's name as the status line says it: a custom server is
 *  "Your server" everywhere. */
export const aiName = (status: AiStatus, t: TFunction) =>
  status.provider_id === "custom"
    ? t("ai.yourServer")
    : status.provider_label || t("ai.provider");

/** One wording for the AI's state on every screen. `allowed` is false when
 *  the feature showing the line is switched off in Settings › AI. */
export const describeAi = (
  status: AiStatus | null,
  t: TFunction,
  allowed = true,
  formatTime: (unixSeconds: number) => string = (s) =>
    new Date(s * 1000).toLocaleTimeString([], {
      hour: "numeric",
      minute: "2-digit",
    }),
): AiLine => {
  if (!status || status.kind === "none")
    return { dot: "off", text: t("ai.status.none"), action: "setup" };
  const name = aiName(status, t);
  const values = { name, model: status.model };
  switch (status.kind) {
    case "unreachable":
      return {
        dot: "danger",
        text:
          status.provider_id === "ollama"
            ? t("ai.status.unreachableOllama")
            : t("ai.status.unreachable", values),
        action: "retry",
      };
    case "rejected":
      return status.has_key
        ? {
            dot: "danger",
            text: t("ai.status.rejectedKey", values),
            action: "fix",
          }
        : {
            dot: "danger",
            text: t("ai.status.rejected", values),
            action: "retry",
          };
    case "not_found":
      return {
        dot: "danger",
        text: t("ai.status.notFound", values),
        action: "fix",
      };
    case "rate_limited":
      return {
        dot: "danger",
        text: t("ai.status.rateLimited", values),
        action: "retry",
      };
    case "slow":
      return {
        dot: "danger",
        text: t("ai.status.slow", values),
        action: "retry",
      };
    case "failed":
      return {
        dot: "danger",
        text: t("ai.status.failed", values),
        action: "retry",
      };
  }
  if (!allowed)
    return { dot: "off", text: t("ai.status.off"), action: "change" };
  const line = t("ai.status.ok", {
    ...values,
    where: t(status.local ? "ai.where.local" : "ai.where.cloud"),
  });
  return {
    dot: "ok",
    text: status.tested_at
      ? t("ai.status.tested", { line, time: formatTime(status.tested_at) })
      : line,
    action: null,
  };
};
