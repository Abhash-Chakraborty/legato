import React from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { PhoneIncoming } from "lucide-react";
import { commands } from "@/bindings";
import { useCallsStore } from "../../../stores/callsStore";
import { useNavStore } from "../../../stores/navStore";
import { Button } from "../../ui/Button";
import { useCallSettings } from "../../settings/calls/CallSettings";

/** Whether Legato is watching for calls, the call going on now, and a way
 *  to test the card. The recording strip takes its place while recording. */
export const CallStrip: React.FC = () => {
  const { t, i18n } = useTranslation();
  const state = useCallsStore((s) => s.state);
  const go = useNavStore((s) => s.go);
  const { set } = useCallSettings();
  if (!state) return null;

  const all = state.watched.map((a) => t(`calls.app.${a}`));
  const names =
    all.length > 1
      ? t("notes.calls.list", {
          list: all.slice(0, -1).join(", "),
          last: all[all.length - 1],
        })
      : (all[0] ?? "");

  let dot = "bg-text-4";
  let status: string;
  let help: string;
  let action: React.ReactNode;
  if (!state.detect) {
    status = t("notes.calls.off");
    help = t("notes.calls.offHelp");
    action = (
      <Button variant="secondary" size="sm" onClick={() => set("detect", true)}>
        {t("notes.calls.turnOn")}
      </Button>
    );
  } else if (state.live) {
    dot = "bg-teal-600";
    status = t("notes.calls.live", { app: state.live.name });
    help = t("notes.calls.liveHelp", {
      time: new Date(state.live.since * 1000).toLocaleTimeString(
        i18n.language,
        { hour: "numeric", minute: "2-digit" },
      ),
    });
    action = (
      <Button size="sm" onClick={() => commands.callCardAction("start")}>
        <span aria-hidden="true" className="h-2 w-2 rounded-full bg-danger" />
        {t("notes.calls.start")}
      </Button>
    );
  } else if (!state.watched.length) {
    status = t("notes.calls.none");
    help = t("notes.calls.noneHelp");
    action = (
      <Button
        variant="secondary"
        size="sm"
        onClick={() => go("settings", "calls")}
      >
        {t("notes.calls.chooseApps")}
      </Button>
    );
  } else {
    status = t("notes.calls.waiting");
    help = t("notes.calls.waitingHelp", { apps: names });
    action = (
      <Button
        variant="ghost"
        size="sm"
        onClick={async () => {
          const result = await commands.testCallCard();
          if (result.status === "error") toast.error(result.error);
        }}
      >
        <PhoneIncoming className="h-3.5 w-3.5" aria-hidden="true" />
        {t("notes.calls.test")}
      </Button>
    );
  }

  return (
    <section
      aria-label={t("notes.calls.label")}
      className={`mb-4 flex min-h-14 flex-wrap items-center gap-x-3 gap-y-2 rounded-[var(--r-lg)] border bg-surface py-2 pe-3 ps-[18px] ${
        state.live && state.detect
          ? "border-[color-mix(in_srgb,var(--teal-600)_45%,var(--border))]"
          : "border-line"
      }`}
    >
      <span className="inline-flex flex-none items-center gap-2 text-sm font-medium">
        <span aria-hidden="true" className={`h-2 w-2 rounded-full ${dot}`} />
        <span role="status">{status}</span>
      </span>
      <span className="min-w-0 flex-1 truncate text-[13px] text-text-3">
        {help}
      </span>
      <span className="flex-none">{action}</span>
    </section>
  );
};
