import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { openUrl } from "@tauri-apps/plugin-opener";
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

const savedSite = (): CalendarSite => {
  try {
    const site = localStorage.getItem(SITE_KEY);
    if (site === "google" || site === "outlook" || site === "outlookCom")
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

/** Schedule a meeting without signing in anywhere: the calendar's own
 *  new-event page opens in the browser with the title, time and invites
 *  filled in, and you save it there. */
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
  const [site, setSite] = useState<CalendarSite>(savedSite);
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
  }, [open]);

  const emails = parseInvite(invite);
  const start = new Date(`${date}T${time}`);
  const ready = !Number.isNaN(start.getTime());

  const go = async () => {
    if (!ready) return;
    try {
      localStorage.setItem(SITE_KEY, site);
    } catch {
      // Remembering the calendar is only a convenience.
    }
    const meeting: NewMeeting = {
      title: title.trim() || t("notes.schedule.untitled"),
      start,
      minutes: Number(length),
      invite: emails,
      details: prefillRef.current?.details,
    };
    try {
      await openUrl(scheduleUrl(site, meeting));
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
          <Button onClick={go} disabled={!ready}>
            {t(`notes.schedule.open.${site}`)}
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
            value={site}
            onChange={setSite}
            options={(["google", "outlook", "outlookCom"] as const).map(
              (s) => ({ value: s, label: t(`notes.schedule.site.${s}`) }),
            )}
          />
          <span className="text-[13px] text-text-3">
            {t(`notes.schedule.siteHelp.${site}`)}
          </span>
        </div>
      </form>
    </Dialog>
  );
};
