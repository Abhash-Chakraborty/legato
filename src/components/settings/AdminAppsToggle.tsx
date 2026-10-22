import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { ToggleSwitch } from "../ui/ToggleSwitch";
import { useSettings } from "../../hooks/useSettings";

/** Work in apps run as administrator. Turning it on or off restarts
 *  Legato with or without administrator rights; Windows asks the first time. */
export const AdminAppsToggle: React.FC<{ grouped?: boolean }> = React.memo(
  ({ grouped = false }) => {
    const { t } = useTranslation();
    const { getSetting, refreshSettings } = useSettings();
    const [busy, setBusy] = useState(false);
    // Why it can't be turned on here: not an admin, or not installed for
    // all users, where no other program can swap the exe.
    const [unavailable, setUnavailable] = useState<string | null>(null);
    useEffect(() => {
      void commands.adminAppsUnavailable().then(setUnavailable);
    }, []);
    const checked = getSetting("admin_apps") ?? false;
    const blocked = !checked && unavailable !== null;

    const change = async (enabled: boolean) => {
      setBusy(true);
      const r = await commands.changeAdminAppsSetting(enabled);
      // On success Legato restarts; this only runs when it didn't.
      setBusy(false);
      if (r.status === "error") {
        const known = r.error === "notAllUsers" || r.error === "notAdmin";
        toast.error(
          known ? t(`settings.advanced.adminApps.${r.error}`) : r.error,
        );
      }
      refreshSettings();
    };

    return (
      <ToggleSwitch
        checked={checked}
        onChange={change}
        isUpdating={busy}
        disabled={blocked}
        label={t("settings.advanced.adminApps.label")}
        description={
          blocked
            ? t(`settings.advanced.adminApps.${unavailable}`)
            : t("settings.advanced.adminApps.description")
        }
        descriptionMode="inline"
        grouped={grouped}
      />
    );
  },
);
