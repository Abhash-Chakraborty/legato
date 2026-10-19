import React, { useState } from "react";
import { useTranslation } from "react-i18next";
import { Check, Copy, TriangleAlert, X } from "lucide-react";
import { copyToClipboard } from "../pages/home/clipboard";
import { commands, type ScratchNote } from "@/bindings";
import { AppIcon } from "../pages/home/AppIcon";
import { useScratchStore } from "../../stores/scratchStore";

/** Says where a failed paste was going and why it didn't arrive, with Copy
 *  and a way to hide the message. */
export const FailedPasteBanner: React.FC<{
  note: ScratchNote;
  text: string;
}> = ({ note, text }) => {
  const { t } = useTranslation();
  const put = useScratchStore((s) => s.put);
  const [copied, setCopied] = useState(false);
  if (note.source !== "failed_paste" || !note.reason) return null;

  const copy = async () => {
    if (!(await copyToClipboard(text))) return;
    setCopied(true);
    window.setTimeout(() => setCopied(false), 1600);
  };
  const dismiss = async () => {
    const result = await commands.dismissScratchReason(note.id);
    if (result.status === "ok") put(result.data);
  };

  return (
    <div
      role="status"
      className="flex flex-none items-center gap-2.5 bg-warn-soft py-[7px] pe-2 ps-3 text-[13px] leading-[1.4]"
    >
      <TriangleAlert
        className="h-[15px] w-[15px] flex-none text-warn"
        aria-hidden="true"
      />
      <span className="min-w-0 flex-1">
        {note.source_app && (
          <AppIcon
            path={note.source_app_path}
            className="me-1.5 inline h-4 w-4 align-[-3px]"
          />
        )}
        {/* The reason usually names the app; say where it was going when
            it doesn't. */}
        {note.source_app && !note.reason.includes(note.source_app)
          ? t("scratchpad.banner.into", { app: note.source_app }) + " "
          : !note.source_app
            ? t("scratchpad.banner.noApp") + " "
            : ""}
        {note.reason}
      </span>
      <button
        type="button"
        onClick={copy}
        className={`inline-flex h-[26px] flex-none items-center gap-1.5 rounded-[var(--r-sm)] px-2 font-medium transition-colors duration-[var(--dur-1)] hover:bg-warn/15 hover:duration-0 ${copied ? "text-teal-700" : ""}`}
      >
        {copied ? (
          <Check className="h-3.5 w-3.5" aria-hidden="true" />
        ) : (
          <Copy className="h-3.5 w-3.5" aria-hidden="true" />
        )}
        {copied ? t("scratchpad.copied") : t("scratchpad.copy")}
      </button>
      <button
        type="button"
        onClick={dismiss}
        aria-label={t("scratchpad.banner.dismiss")}
        title={t("scratchpad.banner.dismiss")}
        className="grid h-[26px] w-[26px] flex-none place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-warn/15 hover:text-text hover:duration-0"
      >
        <X className="h-3.5 w-3.5" aria-hidden="true" />
      </button>
    </div>
  );
};
