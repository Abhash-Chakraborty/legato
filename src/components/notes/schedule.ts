/** Where a new meeting is made: the calendar's own page in the browser,
 *  filled in, so Legato needs no sign-in to schedule. */
export type CalendarSite = "google" | "outlook" | "outlookCom";

export interface NewMeeting {
  title: string;
  /** Local start, and length in minutes. */
  start: Date;
  minutes: number;
  invite: string[];
  details?: string;
}

/** "20261021T100000Z", the UTC form Google's template takes. */
const googleTime = (d: Date) =>
  d
    .toISOString()
    .replace(/[-:]/g, "")
    .replace(/\.\d{3}/, "");

/** Addresses from a typed list: commas, semicolons, spaces or new lines
 *  between them, and "Name <a@b.c>" as copied from mail. */
export const parseInvite = (text: string): string[] => {
  const out: string[] = [];
  for (const part of text.split(/[,;\s]+/)) {
    const email = part.replace(/^[<("']+|[>)"'.]+$/g, "");
    if (/^[^@\s]+@[^@\s]+\.[^@\s]+$/.test(email)) {
      if (!out.some((e) => e.toLowerCase() === email.toLowerCase()))
        out.push(email);
    }
  }
  return out;
};

/** A query string with spaces as %20, which every reader takes for a
 *  space. URLSearchParams writes "+", which only form decoding turns back
 *  into one; Outlook's mail links show it as it is. */
const query = (q: URLSearchParams) => q.toString().replace(/\+/g, "%20");

/** The page that opens the calendar's new-event form with the meeting in
 *  it. Google adds a Meet link with one click (or on its own, when that's
 *  your setting); Outlook offers Teams the same way. */
export const scheduleUrl = (site: CalendarSite, m: NewMeeting): string => {
  const end = new Date(m.start.getTime() + m.minutes * 60_000);
  if (site === "google") {
    const q = new URLSearchParams({
      action: "TEMPLATE",
      text: m.title,
      dates: `${googleTime(m.start)}/${googleTime(end)}`,
    });
    if (m.invite.length) q.set("add", m.invite.join(","));
    if (m.details) q.set("details", m.details);
    return `https://calendar.google.com/calendar/render?${query(q)}`;
  }
  const host =
    site === "outlook"
      ? "https://outlook.office.com/calendar/deeplink/compose"
      : "https://outlook.live.com/calendar/0/deeplink/compose";
  const q = new URLSearchParams({
    path: "/calendar/action/compose",
    rru: "addevent",
    subject: m.title,
    startdt: m.start.toISOString(),
    enddt: end.toISOString(),
  });
  if (m.invite.length) q.set("to", m.invite.join(","));
  if (m.details) q.set("body", m.details);
  return `${host}?${query(q)}`;
};
