import React, { useState } from "react";
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

    const change = async (enabled: boolean) => {
      setBusy(true);
      const r = await commands.changeAdminAppsSetting(enabled);
      // On success Legato restarts; this only runs when it didn't.
      setBusy(false);
      if (r.status === "error") toast.error(r.error);
      refreshSettings();
    };

    return (
      <ToggleSwitch
        checked={getSetting("admin_apps") ?? false}
        onChange={change}
        isUpdating={busy}
        label={t("settings.advanced.adminApps.label")}
        description={t("settings.advanced.adminApps.description")}
        descriptionMode="inline"
        grouped={grouped}
      />
    );
  },
);
