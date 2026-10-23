import assert from "node:assert/strict";
import { parseInvite, scheduleUrl } from "./schedule";

assert.deepEqual(
  parseInvite(
    "Sahil Khan <sahil@example.com>, priya@example.org; bad@, SAHIL@example.com\nx@y.io.",
  ),
  ["sahil@example.com", "priya@example.org", "x@y.io"],
);
assert.deepEqual(parseInvite(""), []);

const meeting = {
  title: "Follow-up: GIS app",
  start: new Date(Date.UTC(2026, 9, 26, 9, 30)),
  minutes: 45,
  invite: ["sahil@example.com", "priya@example.org"],
  details: "Notes: full width",
};

const google = new URL(scheduleUrl("google", meeting));
assert.equal(
  google.origin + google.pathname,
  "https://calendar.google.com/calendar/render",
);
assert.equal(google.searchParams.get("action"), "TEMPLATE");
assert.equal(google.searchParams.get("text"), "Follow-up: GIS app");
assert.equal(
  google.searchParams.get("dates"),
  "20261026T093000Z/20261026T101500Z",
);
assert.equal(
  google.searchParams.get("add"),
  "sahil@example.com,priya@example.org",
);
assert.equal(google.searchParams.get("details"), "Notes: full width");

const work = new URL(
  scheduleUrl("outlook", { ...meeting, invite: [], details: undefined }),
);
assert.equal(work.host, "outlook.office.com");
assert.equal(work.searchParams.get("subject"), "Follow-up: GIS app");
assert.equal(work.searchParams.get("startdt"), "2026-10-26T09:30:00.000Z");
assert.equal(work.searchParams.get("enddt"), "2026-10-26T10:15:00.000Z");
assert.equal(work.searchParams.get("to"), null);

assert.equal(
  new URL(scheduleUrl("outlookCom", meeting)).host,
  "outlook.live.com",
);
console.log("schedule: ok");
