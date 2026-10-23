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
  return t("settings.general.shortcut.errors.inUse", {
    name: bindingName(t, other, transforms),
  });
}

/** A binding's name as Settings shows it: a transform's own name, or the
 *  translated name of a built-in action. */
export function bindingName(
  t: TFunction,
  id: string,
  transforms: Transform[] | undefined,
): string {
  const transformId = id.startsWith("transform:")
    ? id.slice("transform:".length)
    : null;
  return transformId
    ? (transforms?.find((x) => x.id === transformId)?.name ?? transformId)
    : t(`settings.general.shortcut.bindings.${id}.name`, { defaultValue: id });
}
