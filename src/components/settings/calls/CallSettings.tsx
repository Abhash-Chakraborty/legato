import React, { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { PhoneIncoming } from "lucide-react";
import { commands, type CallApp, type CallSettings } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useNavStore } from "../../../stores/navStore";
import { watchCalls } from "../../../stores/callsStore";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

export const CALL_APPS: CallApp[] = [
  "teams",
  "zoom",
  "meet",
  "slack",
  "discord",
];

const DEFAULTS: Required<Omit<CallSettings, "last">> & {
  last: CallSettings["last"];
} = {
  detect: true,
  browser_titles: true,
  watched: ["teams", "zoom", "meet", "slack"],
  auto_start: false,
  countdown_secs: 10,
  snooze_minutes: 10,
  hide_card_from_capture: true,
  stop_on_end: true,
  max_minutes: 120,
  open_note: false,
  last: null,
};

/** Settings › Context & calls, with the defaults filled in, and a setter
 *  for one field. */
export const useCallSettings = () => {
  const { settings, updateSetting, isUpdating } = useSettings();
  const calls = { ...DEFAULTS, ...settings?.calls };
  const set = <K extends keyof CallSettings>(key: K, value: CallSettings[K]) =>
    updateSetting("calls", { ...calls, [key]: value });
  return { calls, set, busy: isUpdating("calls") };
};

const CallToggle: React.FC<{
  field:
    | "detect"
    | "browser_titles"
    | "auto_start"
    | "hide_card_from_capture"
    | "stop_on_end"
    | "open_note";
  labelKey: string;
}> = ({ field, labelKey }) => {
  const { t } = useTranslation();
  const { calls, set, busy } = useCallSettings();
  return (
    <ToggleSwitch
      checked={calls[field]}
      onChange={(on) => set(field, on)}
      isUpdating={busy}
      label={t(`${labelKey}.title`)}
      description={t(`${labelKey}.help`)}
      descriptionMode="inline"
      grouped
    />
  );
};

export const DetectCallsRow = () => (
  <CallToggle field="detect" labelKey="settings.calls.detect" />
);
export const BrowserTitlesRow = () => (
  <CallToggle field="browser_titles" labelKey="settings.calls.browserTitles" />
);
export const AutoStartRow = () => (
  <CallToggle field="auto_start" labelKey="settings.calls.autoStart" />
);
export const HideCardRow = () => (
  <CallToggle
    field="hide_card_from_capture"
    labelKey="settings.calls.hideCard"
  />
);
export const StopOnEndRow = () => (
  <CallToggle field="stop_on_end" labelKey="settings.calls.stopOnEnd" />
);
export const OpenNoteRow = () => (
  <CallToggle field="open_note" labelKey="settings.calls.openNote" />
);

/** The apps watched for calls, each with how Legato recognises it. */
export const WatchedAppsRow: React.FC = () => {
  const { t } = useTranslation();
  const { calls, set, busy } = useCallSettings();
  const watched = calls.watched;
  const toggle = (app: CallApp, on: boolean) =>
    set(
      "watched",
      on
        ? CALL_APPS.filter((a) => a === app || watched.includes(a))
        : watched.filter((a) => a !== app),
    );
  return (
    <div>
      <SettingContainer
        title={t("settings.calls.apps.title")}
        description={t("settings.calls.apps.help")}
        grouped
      >
        <span className="text-[13px] tabular-nums text-text-2">
          {t("settings.calls.apps.count", {
            count: watched.length,
            total: CALL_APPS.length,
          })}
        </span>
      </SettingContainer>
      <ul className="ms-6 border-s border-line">
        {CALL_APPS.map((app) => {
          const label = t(`calls.app.${app}`);
          const needsTitles = app === "meet" && !calls.browser_titles;
          return (
            <li
              key={app}
              className="flex items-center gap-4 border-b border-line py-3 ps-4 last:border-b-0"
            >
              <div className="min-w-0 flex-1">
                <div
                  className={`text-sm font-medium ${needsTitles ? "opacity-45" : ""}`}
                >
                  {label}
                </div>
                <div className="mt-0.5 text-[13px] text-text-3">
                  {needsTitles ? (
                    t("settings.calls.apps.needsTitles")
                  ) : (
                    <>
                      <span className="font-mono text-xs">
                        {t(`settings.calls.apps.signal.${app}`)}
                      </span>
                      {t(`settings.calls.apps.what.${app}`)}
                    </>
                  )}
                </div>
              </div>
              <button
                type="button"
                role="switch"
                aria-checked={watched.includes(app)}
                aria-label={t("settings.calls.apps.watch", { app: label })}
                aria-busy={busy || undefined}
                disabled={needsTitles}
                className="toggle"
                onClick={() => !busy && toggle(app, !watched.includes(app))}
              />
            </li>
          );
        })}
      </ul>
    </div>
  );
};

/** The last call Legato noticed, and the note taken during it. */
export const LastCallRow: React.FC = () => {
  const { t, i18n } = useTranslation();
  const goTo = useNavStore((s) => s.goTo);
  const { calls } = useCallSettings();
  const last = calls.last;
  let description = t("settings.calls.last.none");
  if (last) {
    const at = new Date(last.at * 1000).toLocaleString(i18n.language, {
      weekday: "short",
      hour: "numeric",
      minute: "2-digit",
    });
    const length = last.minutes
      ? t("settings.calls.last.minutes", { count: last.minutes })
      : t("settings.calls.last.going");
    description = t(
      last.note_id ? "settings.calls.last.line" : "settings.calls.last.noNote",
      { app: t(`calls.app.${last.app}`), at, length },
    );
  }
  return (
    <SettingContainer
      title={t("settings.calls.last.title")}
      description={description}
      grouped
    >
      {last?.note_id != null && (
        <Button
          variant="secondary"
          size="sm"
          onClick={() => goTo("notes", String(last.note_id))}
        >
          {t("settings.calls.last.open")}
        </Button>
      )}
    </SettingContainer>
  );
};

/** Shows the real call card, as if the first watched app started a call. */
export const TestDetectionRow: React.FC = () => {
  const { t } = useTranslation();
  const { calls } = useCallSettings();
  useEffect(watchCalls, []);
  const first = calls.watched[0];
  return (
    <SettingContainer
      title={t("settings.calls.test.title")}
      description={
        first
          ? t("settings.calls.test.help", { app: t(`calls.app.${first}`) })
          : t("settings.calls.test.helpNone")
      }
      grouped
    >
      <Button
        variant="secondary"
        size="sm"
        disabled={!first}
        onClick={async () => {
          const result = await commands.testCallCard();
          if (result.status === "error") toast.error(result.error);
        }}
      >
        <PhoneIncoming className="h-3.5 w-3.5" aria-hidden="true" />
        {t("settings.calls.test.button")}
      </Button>
    </SettingContainer>
  );
};

const COUNTDOWNS = [5, 10, 15, 20, 30];

export const CountdownRow: React.FC = () => {
  const { t } = useTranslation();
  const { calls, set } = useCallSettings();
  const label = t("settings.calls.countdown.title");
  return (
    <SettingContainer
      title={label}
      description={t("settings.calls.countdown.help")}
      grouped
    >
      <Dropdown
        className="w-[200px]"
        ariaLabel={label}
        selectedValue={String(calls.countdown_secs)}
        options={COUNTDOWNS.map((s) => ({
          value: String(s),
          label: t("settings.calls.countdown.option", { count: s }),
        }))}
        onSelect={(v) => set("countdown_secs", Number(v))}
      />
    </SettingContainer>
  );
};

const SNOOZES = [2, 10, 30, 0];

export const SnoozeRow: React.FC = () => {
  const { t } = useTranslation();
  const { calls, set } = useCallSettings();
  const label = t("settings.calls.snooze.title");
  return (
    <SettingContainer
      title={label}
      description={t("settings.calls.snooze.help")}
      grouped
    >
      <Dropdown
        className="w-[240px]"
        ariaLabel={label}
        selectedValue={String(calls.snooze_minutes)}
        options={SNOOZES.map((m) => ({
          value: String(m),
          label: m
            ? t("settings.calls.snooze.minutes", { count: m })
            : t("settings.calls.snooze.call"),
        }))}
        onSelect={(v) => set("snooze_minutes", Number(v))}
      />
    </SettingContainer>
  );
};

const LIMITS = [60, 120, 180, 0];

export const MaxLengthRow: React.FC = () => {
  const { t } = useTranslation();
  const { calls, set } = useCallSettings();
  const label = t("settings.calls.maxLength.title");
  return (
    <SettingContainer
      title={label}
      description={t("settings.calls.maxLength.help")}
      grouped
    >
      <Dropdown
        className="w-[200px]"
        ariaLabel={label}
        selectedValue={String(calls.max_minutes)}
        options={LIMITS.map((m) => ({
          value: String(m),
          label: m
            ? t("settings.calls.maxLength.hours", { count: m / 60 })
            : t("settings.calls.maxLength.none"),
        }))}
        onSelect={(v) => set("max_minutes", Number(v))}
      />
    </SettingContainer>
  );
};
