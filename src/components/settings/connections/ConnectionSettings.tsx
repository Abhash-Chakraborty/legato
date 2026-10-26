import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { CalendarDays, Check, Copy, RefreshCw, Video, X } from "lucide-react";
import {
  commands,
  events,
  type CalendarStatus,
  type McpInfo,
  type Meeting,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { formatRelativeTime } from "../../../utils/dateFormat";
import { Button } from "../../ui/Button";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

/** The calendars and their meetings, kept fresh by the backend's event. */
const useCalendars = () => {
  const [feeds, setFeeds] = useState<CalendarStatus[]>([]);
  const [meetings, setMeetings] = useState<Meeting[]>([]);
  const [loaded, setLoaded] = useState(false);
  useEffect(() => {
    const load = () => {
      commands
        .calendarList()
        .then((f) => {
          setFeeds(f);
          setLoaded(true);
        })
        .catch(() => setLoaded(true));
      commands
        .calendarMeetings()
        .then(setMeetings)
        .catch(() => {});
    };
    load();
    const off = events.calendarChanged.listen(load);
    return () => {
      off.then((f) => f());
    };
  }, []);
  return { feeds, meetings, loaded };
};

const time = (secs: number, locale: string) =>
  new Date(secs * 1000).toLocaleTimeString(locale, {
    hour: "numeric",
    minute: "2-digit",
  });

const day = (secs: number, locale: string) => {
  const d = new Date(secs * 1000);
  const today = new Date();
  const tomorrow = new Date();
  tomorrow.setDate(today.getDate() + 1);
  if (d.toDateString() === today.toDateString()) return "today";
  if (d.toDateString() === tomorrow.toDateString()) return "tomorrow";
  return d.toLocaleDateString(locale, {
    weekday: "short",
    month: "short",
    day: "numeric",
  });
};

/** Settings › Connections › Calendars: the list, and adding one by its
 *  secret iCal address. */
export const CalendarsCard: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { feeds } = useCalendars();
  // The calendar list lives in the settings too; Coming up shows from it.
  const { refreshSettings } = useSettings();
  const [url, setUrl] = useState("");
  const [name, setName] = useState("");
  const [adding, setAdding] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [refreshing, setRefreshing] = useState(false);
  const [google, setGoogle] = useState(false);
  const [signingIn, setSigningIn] = useState(false);

  useEffect(() => {
    // The fetch finished when the list comes back.
    setRefreshing(false);
  }, [feeds]);

  useEffect(() => {
    commands
      .googleAvailable()
      .then(setGoogle)
      .catch(() => {});
  }, []);

  const signIn = async () => {
    if (signingIn) return;
    setSigningIn(true);
    setError(null);
    const r = await commands
      .calendarConnectGoogle()
      .catch((e) => ({ status: "error" as const, error: String(e) }));
    setSigningIn(false);
    if (r.status === "error") {
      if (r.error !== "cancelled") setError(r.error);
      return;
    }
    refreshSettings();
    toast.success(
      t("settings.connections.calendars.google.connected", {
        name: r.data.name,
        count: r.data.upcoming,
      }),
    );
  };

  const add = async (address = url, label = name) => {
    if (!address.trim() || adding) return;
    setAdding(true);
    setError(null);
    const r = await commands.calendarAdd(label, address);
    setAdding(false);
    if (r.status === "error") {
      setError(r.error);
      return;
    }
    setUrl("");
    setName("");
    refreshSettings();
    toast.success(
      t("settings.connections.calendars.added", {
        name: r.data.name,
        count: r.data.upcoming,
      }),
    );
  };

  const remove = async (feed: CalendarStatus) => {
    const r = await commands.calendarRemove(feed.id);
    if (r.status === "error") {
      toast.error(r.error);
      return;
    }
    const removed = r.data;
    refreshSettings();
    // A Google account signs out for good; an address can come back.
    toast(
      t("settings.connections.calendars.removed", { name: feed.name }),
      removed.url
        ? {
            action: {
              label: t("settings.devices.undo"),
              onClick: () => add(removed.url, removed.name),
            },
          }
        : undefined,
    );
  };

  return (
    <div>
      <SettingContainer
        title={t("settings.connections.calendars.title")}
        description={t("settings.connections.calendars.help")}
        grouped
      >
        {feeds.length > 0 && (
          <Button
            variant="secondary"
            size="sm"
            disabled={refreshing}
            onClick={() => {
              setRefreshing(true);
              commands.calendarRefresh();
            }}
          >
            <RefreshCw
              className={`h-3.5 w-3.5 ${refreshing ? "animate-spin" : ""}`}
              aria-hidden="true"
            />
            {t("settings.connections.calendars.refresh")}
          </Button>
        )}
      </SettingContainer>
      <ul className="ms-6 border-s border-line">
        {feeds.map((f) => (
          <li
            key={f.id}
            className="group flex items-center gap-3 border-b border-line py-2.5 ps-4"
          >
            <CalendarDays
              className="h-4 w-4 shrink-0 text-text-3"
              aria-hidden="true"
            />
            <div className="min-w-0 flex-1">
              <div className="truncate text-[14px] font-medium">{f.name}</div>
              <div
                className={`truncate text-[12.5px] ${f.error ? "text-warn" : "text-text-3"}`}
              >
                {f.error
                  ? f.error
                  : f.synced_at
                    ? t("settings.connections.calendars.status", {
                        when: formatRelativeTime(
                          String(f.synced_at),
                          i18n.language,
                        ),
                        count: f.upcoming,
                      })
                    : t("settings.connections.calendars.waiting")}
                {f.google &&
                  f.waiting > 0 &&
                  ` · ${t("settings.connections.calendars.google.waiting", {
                    count: f.waiting,
                  })}`}
              </div>
            </div>
            <button
              type="button"
              aria-label={t("settings.connections.calendars.remove", {
                name: f.name,
              })}
              title={t("settings.connections.calendars.remove", {
                name: f.name,
              })}
              onClick={() => remove(f)}
              className="grid h-7 w-7 place-items-center rounded-[var(--r-xs)] text-text-3 opacity-0 transition-opacity duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text group-hover:opacity-100 group-hover:duration-0 focus-visible:opacity-100"
            >
              <X className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
          </li>
        ))}
        {google && (
          <li className="flex flex-wrap items-center gap-x-3 gap-y-1.5 border-b border-line py-2.5 ps-4">
            {signingIn ? (
              <>
                <span className="text-[13px] text-text-2" role="status">
                  {t("settings.connections.calendars.google.browser")}
                </span>
                <Button
                  variant="ghost"
                  size="sm"
                  onClick={() => commands.calendarCancelGoogle()}
                >
                  {t("common.cancel")}
                </Button>
              </>
            ) : (
              <>
                <Button variant="secondary" size="sm" onClick={signIn}>
                  {t("settings.connections.calendars.google.signIn")}
                </Button>
                <span className="max-w-[460px] text-[12.5px] leading-[1.45] text-text-3">
                  {t("settings.connections.calendars.google.help")}
                </span>
              </>
            )}
          </li>
        )}
        <li className="py-2.5 ps-4">
          <form
            className="flex flex-wrap items-center gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              add();
            }}
          >
            <input
              className={`input compact min-w-0 flex-[1_1_260px] ${url ? "font-mono text-[12.5px]" : ""}`}
              value={url}
              placeholder={t("settings.connections.calendars.urlPlaceholder")}
              aria-label={t("settings.connections.calendars.url")}
              spellCheck={false}
              onChange={(e) => {
                setUrl(e.target.value);
                setError(null);
              }}
              onKeyDown={(e) => {
                if (e.key === "Escape") {
                  setUrl("");
                  setError(null);
                }
              }}
            />
            <input
              className="input compact w-40"
              value={name}
              maxLength={40}
              placeholder={t("settings.connections.calendars.namePlaceholder")}
              aria-label={t("settings.connections.calendars.name")}
              onChange={(e) => setName(e.target.value)}
            />
            <Button
              type="submit"
              variant="secondary"
              size="sm"
              disabled={!url.trim() || adding}
            >
              {adding
                ? t("settings.connections.calendars.checking")
                : t("settings.connections.calendars.add")}
            </Button>
          </form>
          {error ? (
            <p className="mt-1.5 text-[12.5px] text-warn" role="alert">
              {error}
            </p>
          ) : (
            <p className="mt-1.5 max-w-[560px] text-[12.5px] leading-[1.45] text-text-3">
              {t("settings.connections.calendars.where")}
            </p>
          )}
        </li>
      </ul>
    </div>
  );
};

/** The next few meetings, so it's plain the calendars are read. */
export const ComingUpRow: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { meetings } = useCalendars();
  const now = Date.now() / 1000;
  const next = meetings.filter((m) => !m.all_day && m.end > now).slice(0, 4);
  return (
    <SettingContainer
      title={t("settings.connections.comingUp.title")}
      description={t("settings.connections.comingUp.help")}
      layout="stacked"
      grouped
    >
      {next.length === 0 ? (
        <p className="text-[13px] text-text-3">
          {t("settings.connections.comingUp.none")}
        </p>
      ) : (
        <ul className="mt-1 divide-y divide-line">
          {next.map((m) => (
            <li
              key={`${m.uid}-${m.start}`}
              className="flex items-center gap-3 py-2"
            >
              <span className="w-[132px] shrink-0 text-[13px] tabular-nums text-text-2">
                {m.start <= now
                  ? t("settings.connections.comingUp.now")
                  : t("settings.connections.comingUp.when", {
                      day: t(
                        `settings.connections.comingUp.days.${day(m.start, i18n.language)}`,
                        { defaultValue: day(m.start, i18n.language) },
                      ),
                      time: time(m.start, i18n.language),
                    })}
              </span>
              <span className="min-w-0 flex-1 truncate text-[14px]">
                {m.title}
              </span>
              <span className="hidden shrink-0 text-[12.5px] text-text-3 sm:inline">
                {m.calendar}
              </span>
              {m.link && (
                <Video
                  className="h-3.5 w-3.5 shrink-0 text-text-3"
                  aria-label={t("settings.connections.comingUp.hasLink")}
                />
              )}
            </li>
          ))}
        </ul>
      )}
    </SettingContainer>
  );
};

export const hasCalendars = (calendars: unknown[] | undefined) =>
  (calendars?.length ?? 0) > 0;

/** Whether AI apps may read Legato over MCP. */
export const McpToggle: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  return (
    <ToggleSwitch
      checked={getSetting("mcp_enabled") ?? false}
      onChange={(on) => updateSetting("mcp_enabled", on)}
      isUpdating={isUpdating("mcp_enabled")}
      label={t("settings.connections.mcp.title")}
      description={t("settings.connections.mcp.help")}
      descriptionMode="inline"
      grouped
    />
  );
};

/** Adding Legato to the AI apps found on this PC, or copying the setup for
 *  any other. */
export const McpSetupRow: React.FC = () => {
  const { t } = useTranslation();
  const [info, setInfo] = useState<McpInfo | null>(null);
  const [copied, setCopied] = useState(false);
  const load = () =>
    commands
      .mcpInfo()
      .then(setInfo)
      .catch(() => {});
  useEffect(() => {
    load();
  }, []);

  const addTo = async (id: string, name: string) => {
    const r = await commands.mcpAddTo(id);
    if (r.status === "error") {
      toast.error(r.error);
      return;
    }
    toast.success(t("settings.connections.mcp.addedTo", { name }));
    load();
  };

  const copy = async () => {
    if (!info) return;
    await navigator.clipboard.writeText(info.config);
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };

  return (
    <SettingContainer
      title={t("settings.connections.mcp.setup.title")}
      description={t("settings.connections.mcp.setup.help")}
      grouped
    >
      <div className="flex flex-wrap items-center justify-end gap-2">
        {info?.clients.map((c) =>
          c.added ? (
            <span
              key={c.id}
              className="inline-flex h-8 items-center gap-1.5 px-2 text-[13px] text-text-2"
            >
              <Check className="h-3.5 w-3.5 text-ok" aria-hidden="true" />
              {t("settings.connections.mcp.inApp", { name: c.name })}
            </span>
          ) : (
            <Button
              key={c.id}
              variant="secondary"
              size="sm"
              onClick={() => addTo(c.id, c.name)}
            >
              {t("settings.connections.mcp.addTo", { name: c.name })}
            </Button>
          ),
        )}
        <Button variant="ghost" size="sm" onClick={copy} disabled={!info}>
          {copied ? (
            <Check className="h-3.5 w-3.5" aria-hidden="true" />
          ) : (
            <Copy className="h-3.5 w-3.5" aria-hidden="true" />
          )}
          {copied
            ? t("settings.connections.mcp.copied")
            : t("settings.connections.mcp.copy")}
        </Button>
      </div>
    </SettingContainer>
  );
};
