import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import { GlobalShortcutInput } from "./GlobalShortcutInput";
import { HandyKeysShortcutInput } from "./HandyKeysShortcutInput";
import { bindingName } from "./shortcutError";

interface ShortcutInputProps {
  descriptionMode?: "inline" | "tooltip";
  grouped?: boolean;
  shortcutId: string;
  disabled?: boolean;
  title?: string;
  description?: string;
}

/**
 * Wrapper component that selects the appropriate shortcut input implementation
 * based on the keyboard_implementation setting.
 *
 * - "tauri" (default): Uses GlobalShortcutInput with JS keyboard events
 * - "handy_keys": Uses HandyKeysShortcutInput with backend key events
 *
 * A shortcut that another action already has (settings saved before
 * shortcuts were checked for this) does nothing; its help text says so.
 */
export const ShortcutInput: React.FC<ShortcutInputProps> = (props) => {
  const { t } = useTranslation();
  const { getSetting } = useSettings();
  const keyboardImplementation = getSetting("keyboard_implementation");
  const bindings = getSetting("bindings");
  const [shadowedBy, setShadowedBy] = useState<string | undefined>();

  useEffect(() => {
    let live = true;
    commands
      .shadowedShortcuts()
      .then((shadowed) => {
        if (live) setShadowedBy(shadowed[props.shortcutId]);
      })
      .catch(() => {});
    return () => {
      live = false;
    };
  }, [bindings, props.shortcutId]);

  const inputProps = shadowedBy
    ? {
        ...props,
        description: t("settings.general.shortcut.shadowed", {
          name: bindingName(t, shadowedBy, getSetting("transforms")),
        }),
      }
    : props;

  // Default to Tauri implementation if not set
  if (keyboardImplementation === "handy_keys") {
    return <HandyKeysShortcutInput {...inputProps} />;
  }

  return <GlobalShortcutInput {...inputProps} />;
};
