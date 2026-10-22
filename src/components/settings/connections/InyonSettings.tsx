import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { listen } from "@tauri-apps/api/event";
import { commands, type InyonStatus } from "@/bindings";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

/** Inyon's pairing and connection, kept fresh by the backend's event. */
const useInyon = () => {
  const [status, setStatus] = useState<InyonStatus | null>(null);
  useEffect(() => {
    commands
      .inyonStatus()
      .then(setStatus)
      .catch(() => {});
    const off = listen<InyonStatus>("inyon-status", (e) =>
      setStatus(e.payload),
    );
    return () => {
      off.then((f) => f());
    };
  }, []);
  return [status, setStatus] as const;
};

/** Settings › Connections › Inyon: pair with a code, then the hub, whether
 *  it's connected, and Remove. */
export const InyonCard: React.FC = () => {
  const { t } = useTranslation();
  const [status, setStatus] = useInyon();
  const [hub, setHub] = useState("");
  const [code, setCode] = useState("");
  const [pairing, setPairing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const pair = async () => {
    if (!hub.trim() || !code.trim() || pairing) return;
    setPairing(true);
    setError(null);
    const r = await commands.inyonPair(hub, code);
    setPairing(false);
    if (r.status === "error") {
      setError(t(`inyon.reason.${r.error}`, t("inyon.reason.failed")));
      return;
    }
    setStatus(r.data);
    setHub("");
    setCode("");
    toast.success(
      t("settings.connections.inyon.paired", {
        hub: r.data.hub_name ?? r.data.hub ?? "",
      }),
    );
  };

  if (status?.paired) {
    const state = !status.alerts
      ? t("settings.connections.inyon.alertsOff")
      : status.connected
        ? status.waiting
          ? t("settings.connections.inyon.waiting", { count: status.waiting })
          : t("settings.connections.inyon.connected")
        : t("settings.connections.inyon.connecting");
    return (
      <SettingContainer
        title={status.hub_name ?? t("settings.connections.inyon.title")}
        description={`${status.hub ?? ""} · ${state}`}
        grouped
      >
        <Button
          variant="secondary"
          size="sm"
          onClick={() => {
            commands.inyonUnpair();
            toast(t("settings.connections.inyon.removed"));
          }}
        >
          {t("settings.connections.inyon.remove")}
        </Button>
      </SettingContainer>
    );
  }

  return (
    <div>
      <SettingContainer
        title={t("settings.connections.inyon.title")}
        description={t("settings.connections.inyon.help")}
        grouped
      >
        <span />
      </SettingContainer>
      <div className="ms-6 border-s border-line py-2.5 ps-4">
        <form
          className="flex flex-wrap items-center gap-2"
          onSubmit={(e) => {
            e.preventDefault();
            pair();
          }}
        >
          <input
            className={`input compact min-w-0 flex-[1_1_240px] ${hub ? "font-mono text-[12.5px]" : ""}`}
            value={hub}
            placeholder={t("settings.connections.inyon.hubPlaceholder")}
            aria-label={t("settings.connections.inyon.hub")}
            spellCheck={false}
            onChange={(e) => {
              setHub(e.target.value);
              setError(null);
            }}
          />
          <input
            className="input compact w-32 font-mono uppercase"
            value={code}
            maxLength={8}
            placeholder={t("settings.connections.inyon.codePlaceholder")}
            aria-label={t("settings.connections.inyon.code")}
            spellCheck={false}
            autoComplete="off"
            onChange={(e) => {
              setCode(e.target.value);
              setError(null);
            }}
          />
          <Button
            type="submit"
            variant="secondary"
            size="sm"
            disabled={!hub.trim() || !code.trim() || pairing}
          >
            {pairing
              ? t("settings.connections.inyon.pairing")
              : t("settings.connections.inyon.pair")}
          </Button>
        </form>
        {error ? (
          <p className="mt-1.5 text-[12.5px] text-warn" role="alert">
            {error}
          </p>
        ) : (
          <p className="mt-1.5 max-w-[560px] text-[12.5px] leading-[1.45] text-text-3">
            {t("settings.connections.inyon.where")}
          </p>
        )}
      </div>
    </div>
  );
};

/** Agents waiting for you on the bar or the island. */
export const InyonAlertsRow: React.FC = () => {
  const { t } = useTranslation();
  const [status, setStatus] = useInyon();
  return (
    <ToggleSwitch
      checked={status?.alerts ?? true}
      onChange={(on) => {
        commands.inyonSetAlerts(on);
        if (status) setStatus({ ...status, alerts: on });
      }}
      label={t("settings.connections.inyon.alerts.title")}
      description={t("settings.connections.inyon.alerts.help")}
      descriptionMode="inline"
      grouped
    />
  );
};
