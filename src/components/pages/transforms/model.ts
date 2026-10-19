import type { TFunction } from "i18next";
import type { AiStatus, PostProcessProvider, Transform } from "@/bindings";
import { aiName } from "../../../lib/utils/aiStatus";

const isLocalUrl = (url: string) =>
  /^(https?:\/\/)?(localhost|127\.|\[::1\])/i.test(url.trim());

/** Whether text sent to this provider stays on this PC. */
export const providerIsLocal = (provider: PostProcessProvider) =>
  provider.id === "ollama" || isLocalUrl(provider.base_url);

/** Where a transform sends text, as its card says it: "Ollama · qwen2.5:7b ·
 *  local". A transform without its own model uses Settings › AI. */
export const modelLine = (
  transform: Transform,
  providers: PostProcessProvider[],
  status: AiStatus | null,
  t: TFunction,
): string => {
  if (transform.provider_id && transform.model) {
    const provider = providers.find((p) => p.id === transform.provider_id);
    if (provider) {
      const name =
        provider.id === "custom" ? t("ai.yourServer") : provider.label;
      return t("ai.status.ok", {
        name,
        model: transform.model,
        where: t(
          providerIsLocal(provider) ? "ai.where.local" : "ai.where.cloud",
        ),
      });
    }
  }
  if (!status || status.kind === "none") return t("transforms.model.none");
  return t("ai.status.ok", {
    name: aiName(status, t),
    model: status.model,
    where: t(status.local ? "ai.where.local" : "ai.where.cloud"),
  });
};

/** The provider a transform sends text to, for naming it in a problem. */
export const providerFor = (
  transform: Transform,
  providers: PostProcessProvider[],
  status: AiStatus | null,
): { id: string; label: string } | null => {
  const own =
    transform.provider_id && transform.model
      ? providers.find((p) => p.id === transform.provider_id)
      : undefined;
  if (own) return { id: own.id, label: own.label };
  return status
    ? { id: status.provider_id, label: status.provider_label }
    : null;
};
