import React from "react";
import { useTranslation } from "react-i18next";
import { commands } from "@/bindings";
import { useCallsStore } from "../../../stores/callsStore";
import { Button } from "../../ui/Button";

/** A call going on now that isn't being recorded, with Start notes. Shows
 *  nothing otherwise: watching for calls and testing that live in Settings ›
 *  Context & calls. The recording strip takes its place while recording. */
export const CallStrip: React.FC = () => {
  const { t, i18n } = useTranslation();
  const state = useCallsStore((s) => s.state);
  if (!state?.detect || !state.live) return null;
  const live = state.live;

  return (
    <section
      aria-label={t("notes.calls.label")}
      className="mb-4 flex min-h-14 flex-wrap items-center gap-x-3 gap-y-2 rounded-[var(--r-lg)] border border-[color-mix(in_srgb,var(--teal-600)_45%,var(--border))] bg-surface py-2 pe-3 ps-[18px]"
    >
      <span className="inline-flex flex-none items-center gap-2 text-sm font-medium">
        <span aria-hidden="true" className="h-2 w-2 rounded-full bg-teal-600" />
        <span role="status">{t("notes.calls.live", { app: live.name })}</span>
      </span>
      <span className="min-w-0 flex-1 truncate text-[13px] text-text-3">
        {t("notes.calls.liveHelp", {
          time: new Date(live.since * 1000).toLocaleTimeString(i18n.language, {
            hour: "numeric",
            minute: "2-digit",
          }),
        })}
      </span>
      <span className="flex-none">
        <Button size="sm" onClick={() => commands.callCardAction("start")}>
          <span aria-hidden="true" className="h-2 w-2 rounded-full bg-danger" />
          {t("notes.calls.start")}
        </Button>
      </span>
    </section>
  );
};
