import type { TFunction } from "i18next";
import type { Transform } from "@/bindings";

/** The toast for a shortcut that couldn't be saved. When another action
 *  already has those keys, the backend answers "in-use:<id>"; name it. */
export function shortcutErrorText(
  t: TFunction,
  error: unknown,
  transforms: Transform[] | undefined,
): string {
  const other = /in-use:(\S+)/.exec(String(error))?.[1];
  if (!other) {
    return t("settings.general.shortcut.errors.set", { error: String(error) });
  }
  const transformId = other.startsWith("transform:")
    ? other.slice("transform:".length)
    : null;
  const name = transformId
    ? (transforms?.find((x) => x.id === transformId)?.name ?? transformId)
    : t(`settings.general.shortcut.bindings.${other}.name`, {
        defaultValue: other,
      });
  return t("settings.general.shortcut.errors.inUse", { name });
}
