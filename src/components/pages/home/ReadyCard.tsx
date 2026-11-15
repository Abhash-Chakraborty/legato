import React, { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { toast } from "sonner";
import { AudioLines, Copy, Mic, NotebookPen } from "@/icons";
import { commands, type HistoryEntry } from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useModelStore } from "../../../stores/modelStore";
import { useModelStatusStore } from "../../../stores/modelStatusStore";
import { useNavStore } from "../../../stores/navStore";
import { formatKeyCombination } from "../../../lib/utils/keyboard";
import { getTranslatedModelName } from "../../../lib/utils/modelTranslation";
import { Button } from "../../ui/Button";
import { copyToClipboard } from "./clipboard";
import { finalText } from "./format";

/** Whether Legato is listening right now, from the microphone levels a
 *  dictation sends (Home's rail asks for them while the window shows). */
const useListening = () => {
  const [on, setOn] = useState(false);
  useEffect(() => {
    let off: ReturnType<typeof setTimeout> | undefined;
    const unlisten = listen("mic-level", () => {
      setOn(true);
      clearTimeout(off);
      off = setTimeout(() => setOn(false), 500);
    });
    return () => {
      clearTimeout(off);
      unlisten.then((fn) => fn());
    };
  }, []);
  return on;
};

/**
 * The top of Home: how to dictate, in keycaps that go down while you do;
 * the model that will write it up; and what you most often want next. It
 * says at any window width what the rail says only on a wide one.
 */
export const ReadyCard: React.FC<{ last: HistoryEntry | undefined }> = ({
  last,
}) => {
  const { t } = useTranslation();
  const { settings } = useSettings();
  const go = useNavStore((s) => s.go);
  const { models, currentModel } = useModelStore();
  const status = useModelStatusStore((s) => s.status);
  const listening = useListening();

  const binding = settings?.bindings?.transcribe?.current_binding ?? "";
  const keys = formatKeyCombination(binding).split(" + ").filter(Boolean);
  const activation = settings?.shortcut_activation ?? "hold_or_toggle";
  const model = models.find((m) => m.id === currentModel);
  const modelLine = model
    ? `${getTranslatedModelName(model, t)} · ${t(
        status === "ready"
          ? "home.rail.modelLoaded"
          : status === "loading"
            ? "home.rail.modelLoading"
            : status === "error"
              ? "home.rail.modelError"
              : "home.rail.modelIdle",
      )}`
    : t("home.rail.noModel");

  const copyLast = async () => {
    if (last && (await copyToClipboard(finalText(last))))
      toast(t("home.ready.copied"));
  };

  return (
    <section
      aria-label={t("home.ready.label")}
      className="mb-5 flex flex-wrap items-center gap-x-4 gap-y-3 rounded-[var(--r-lg)] border border-line bg-surface-2 px-4 py-3.5"
    >
      <span
        className={`tile lg on ${listening ? "listening" : ""}`}
        aria-hidden="true"
      >
        {listening ? (
          <AudioLines className="h-4 w-4" />
        ) : (
          <Mic className="h-4 w-4" />
        )}
      </span>
      <div className="min-w-0 flex-1 basis-[240px]">
        <p
          className="flex flex-wrap items-center gap-x-1.5 gap-y-1 text-[15px] font-medium"
          aria-live="polite"
        >
          {keys.length ? (
            <>
              {!listening && t(`home.ready.how.${activation}.before`)}
              <span className="inline-flex items-center gap-1">
                {keys.map((key) => (
                  <kbd key={key} className={`kbd ${listening ? "down" : ""}`}>
                    {key}
                  </kbd>
                ))}
              </span>
              {listening
                ? t("home.ready.listening")
                : t(`home.ready.how.${activation}.after`)}
            </>
          ) : listening ? (
            t("home.ready.listening")
          ) : (
            t("home.ready.noShortcut")
          )}
        </p>
        <button
          type="button"
          onClick={() => go("models")}
          className="mt-0.5 max-w-full truncate text-[13px] text-text-3 underline-offset-2 hover:text-text-2 hover:underline"
        >
          {modelLine}
        </button>
      </div>
      <div className="flex flex-none flex-wrap items-center gap-2">
        {last && (
          <Button variant="secondary" size="sm" onClick={copyLast}>
            <Copy className="h-3.5 w-3.5" aria-hidden="true" />
            {t("home.ready.copyLast")}
          </Button>
        )}
        {(settings?.scratchpad_enabled ?? true) && (
          <Button
            variant="secondary"
            size="sm"
            onClick={() => commands.openScratchpad(null).catch(() => {})}
          >
            <NotebookPen className="h-3.5 w-3.5" aria-hidden="true" />
            {t("home.ready.scratchpad")}
          </Button>
        )}
        {!keys.length && (
          <Button size="sm" onClick={() => go("settings", "shortcuts")}>
            {t("home.ready.setShortcut")}
          </Button>
        )}
      </div>
    </section>
  );
};
