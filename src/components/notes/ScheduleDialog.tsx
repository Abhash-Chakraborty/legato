import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
import { commands, type CalendarStatus } from "@/bindings";
import { copyToClipboard } from "../pages/home/clipboard";
import { Button } from "../ui/Button";
import { Dialog } from "../ui/Dialog";
import { Segmented } from "../ui/Segmented";
import {
  parseInvite,
  scheduleUrl,
  type CalendarSite,
  type NewMeeting,
} from "./schedule";

const SITE_KEY = "legato:schedule-site";
const LENGTHS = ["30", "45", "60"] as const;
type Length = (typeof LENGTHS)[number];

/** A calendar's page in the browser, or "direct": straight into a
 *  signed-in Google account. */
type Where = CalendarSite | "direct";

const savedSite = (): Where => {
  try {
    const site = localStorage.getItem(SITE_KEY);
    if (
      site === "direct" ||
      site === "google" ||
      site === "outlook" ||
      site === "outlookCom"
    )
      return site;
  } catch {
    // No storage: start with Google.
  }
  return "google";
};

const pad = (n: number) => String(n).padStart(2, "0");

/** The next half hour that's at least 15 minutes away. */
const nextSlot = () => {
  const d = new Date(Date.now() + 15 * 60_000);
  d.setSeconds(0, 0);
  d.setMinutes(d.getMinutes() <= 30 ? 30 : 60);
  return d;
};

export interface SchedulePrefill {
  title?: string;
  details?: string;
}

/** Schedule a meeting: straight into a Google account signed in under
 *  Settings › Connections (with a Meet link, waiting for the network when
 *  offline), or without signing in anywhere: the calendar's own new-event
 *  page opens in the browser with the title, time and invites filled in,
 *  and you save it there. */
export const ScheduleDialog: React.FC<{
  open: boolean;
  prefill?: SchedulePrefill;
  onClose: () => void;
}> = ({ open, prefill, onClose }) => {
  const { t } = useTranslation();
  const [title, setTitle] = useState("");
  const [date, setDate] = useState("");
  const [time, setTime] = useState("");
  const [length, setLength] = useState<Length>("30");
  const [invite, setInvite] = useState("");
  const [site, setSite] = useState<Where>(savedSite);
  const [accounts, setAccounts] = useState<CalendarStatus[]>([]);
  const [meet, setMeet] = useState(true);
  const [busy, setBusy] = useState(false);
  const titleRef = useRef<HTMLInputElement>(null);
  // Read when the dialog opens only: the note behind it re-renders often.
  const prefillRef = useRef(prefill);
  prefillRef.current = prefill;

  useEffect(() => {
    if (!open) return;
    const slot = nextSlot();
    setTitle(prefillRef.current?.title ?? "");
    setDate(
      `${slot.getFullYear()}-${pad(slot.getMonth() + 1)}-${pad(slot.getDate())}`,
    );
    setTime(`${pad(slot.getHours())}:${pad(slot.getMinutes())}`);
    setLength("30");
    setInvite("");
    setMeet(true);
    commands
      .calendarList()
      .then((feeds) => setAccounts(feeds.filter((f) => f.google)))
      .catch(() => setAccounts([]));
  }, [open]);

  // Without a Google account, "direct" falls back to Google's page.
  const where: Where = site === "direct" && !accounts.length ? "google" : site;
  const account = accounts[0];

  const emails = parseInvite(invite);
  const start = new Date(`${date}T${time}`);
  const ready = !Number.isNaN(start.getTime());

  const addNow = async () => {
    if (!account || busy) return;
    setBusy(true);
    const r = await commands
      .calendarAddMeeting(account.id, {
        title: title.trim() || t("notes.schedule.untitled"),
        start: Math.floor(start.getTime() / 1000),
        minutes: Number(length),
        invite: emails,
        details: prefillRef.current?.details ?? null,
        meet,
      })
      .catch((e) => ({ status: "error" as const, error: String(e) }));
    setBusy(false);
    if (r.status === "error") {
      toast.error(t("notes.schedule.addFailed"), { description: r.error });
      return;
    }
    onClose();
    const added = r.data;
    if (added.waiting) {
      toast(t("notes.schedule.waiting"));
      return;
    }
    const meetLink = added.meet;
    const link = added.link;
    toast.success(t("notes.schedule.added", { account: account.name }), {
      action: meetLink
        ? {
            label: t("notes.schedule.copyMeet"),
            onClick: () => {
              copyToClipboard(meetLink);
            },
          }
        : link
          ? {
              label: t("notes.schedule.openEvent"),
              onClick: () => {
                openUrl(link).catch(() => {});
              },
            }
          : undefined,
    });
  };

  const go = async () => {
    if (!ready) return;
    try {
      localStorage.setItem(SITE_KEY, where);
    } catch {
      // Remembering the calendar is only a convenience.
    }
    if (where === "direct") {
      addNow();
      return;
    }
    const meeting: NewMeeting = {
      title: title.trim() || t("notes.schedule.untitled"),
      start,
      minutes: Number(length),
      invite: emails,
      details: prefillRef.current?.details,
    };
    try {
      await openUrl(scheduleUrl(where, meeting));
      onClose();
    } catch (e) {
      toast.error(t("notes.schedule.failed"), { description: String(e) });
    }
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={t("notes.schedule.title")}
      description={t("notes.schedule.description")}
      closeLabel={t("common.cancel")}
      initialFocusRef={titleRef}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            {t("common.cancel")}
          </Button>
          <Button onClick={go} disabled={!ready || busy}>
            {where === "direct"
              ? t("notes.schedule.addNow")
              : t(`notes.schedule.open.${where}`)}
          </Button>
        </div>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          go();
        }}
        onKeyDown={(e) => {
          if (e.key === "Enter" && e.ctrlKey) {
            e.preventDefault();
            go();
          }
        }}
        className="grid gap-4"
      >
        <label className="grid gap-1.5">
          <span className="text-sm font-medium">
            {t("notes.schedule.titleLabel")}
          </span>
          <input
            ref={titleRef}
            value={title}
            onChange={(e) => setTitle(e.target.value)}
            placeholder={t("notes.schedule.untitled")}
            className="input w-full"
          />
        </label>
        <div className="grid grid-cols-2 gap-3">
          <label className="grid gap-1.5">
            <span className="text-sm font-medium">
              {t("notes.schedule.date")}
            </span>
            <input
              type="date"
              value={date}
              onChange={(e) => setDate(e.target.value)}
              className="input w-full"
            />
          </label>
          <label className="grid gap-1.5">
            <span className="text-sm font-medium">
              {t("notes.schedule.time")}
            </span>
            <input
              type="time"
              value={time}
              step={300}
              onChange={(e) => setTime(e.target.value)}
              className="input w-full"
            />
          </label>
        </div>
        <div className="grid gap-1.5">
          <span className="text-sm font-medium">
            {t("notes.schedule.length")}
          </span>
          <Segmented
            className="w-full"
            label={t("notes.schedule.length")}
            value={length}
            onChange={setLength}
            options={LENGTHS.map((m) => ({
              value: m,
              label: t("notes.schedule.minutes", { count: Number(m) }),
            }))}
          />
        </div>
        <label className="grid gap-1.5">
          <span className="text-sm font-medium">
            {t("notes.schedule.invite")}
          </span>
          <textarea
            value={invite}
            onChange={(e) => setInvite(e.target.value)}
            placeholder={t("notes.schedule.invitePlaceholder")}
            rows={2}
            className="input h-[60px] w-full resize-none py-2"
          />
          <span className="text-[13px] text-text-3">
            {emails.length
              ? t("notes.schedule.inviteCount", { count: emails.length })
              : t("notes.schedule.inviteHelp")}
          </span>
        </label>
        <div className="grid gap-1.5">
          <span className="text-sm font-medium">
            {t("notes.schedule.calendar")}
          </span>
          <Segmented
            className="w-full"
            label={t("notes.schedule.calendar")}
            value={where}
            onChange={setSite}
            options={[
              ...(account
                ? [
                    {
                      value: "direct" as const,
                      label: t("notes.schedule.site.direct"),
                    },
                  ]
                : []),
              ...(["google", "outlook", "outlookCom"] as const).map((s) => ({
                value: s,
                label: t(`notes.schedule.site.${s}`),
              })),
            ]}
          />
          <span className="text-[13px] text-text-3">
            {where === "direct" && account
              ? t("notes.schedule.siteHelp.direct", { account: account.name })
              : t(`notes.schedule.siteHelp.${where}`)}
          </span>
          {where === "direct" && (
            <label className="mt-1 flex items-center gap-2 text-sm">
              <input
                type="checkbox"
                className="check"
                checked={meet}
                onChange={(e) => setMeet(e.target.checked)}
              />
              {t("notes.schedule.meet")}
            </label>
          )}
        </div>
      </form>
    </Dialog>
  );
};
