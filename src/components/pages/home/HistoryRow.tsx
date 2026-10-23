import React, { useCallback, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Check,
  Copy,
  Download,
  EllipsisVertical,
  Pause,
  Play,
  RotateCcw,
  Smartphone,
  SpellCheck,
  Star,
  Trash2,
  type LucideIcon,
} from "lucide-react";
import type { HistoryEntry } from "@/bindings";
import { AudioPlayer } from "../../ui/AudioPlayer";
import { ActionMenu, type ActionMenuItem } from "../../ui/ActionMenu";
import { Tooltip } from "../../ui/Tooltip";
import { markFixes } from "../../../lib/utils/words";
import { AppIcon } from "./AppIcon";
import { FixWord } from "./FixWord";
import { finalText, formatSpeech } from "./format";

export interface RowActions {
  copy: (entry: HistoryEntry, text: string) => Promise<boolean>;
  toggleStar: (entry: HistoryEntry) => void;
  remove: (entry: HistoryEntry) => void;
  retry: (entry: HistoryEntry) => Promise<void>;
  saveAudio: (entry: HistoryEntry) => void;
  loadAudio: (entry: HistoryEntry) => Promise<string | null>;
}

const RowButton: React.FC<{
  label: string;
  icon: LucideIcon;
  onClick: () => void;
  pressed?: boolean;
  expanded?: boolean;
  shortcut?: string;
}> = ({ label, icon: Icon, onClick, pressed, expanded, shortcut }) => (
  <Tooltip label={label} shortcut={shortcut} side="left">
    <button
      type="button"
      onClick={onClick}
      aria-label={label}
      aria-keyshortcuts={shortcut}
      aria-pressed={pressed}
      aria-expanded={expanded}
      aria-haspopup={expanded === undefined ? undefined : "menu"}
      className={`grid h-8 w-8 place-items-center rounded-[var(--r-sm)] transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text hover:duration-0 active:scale-[0.97] ${pressed ? "bg-surface-3 text-text" : "text-text-2"}`}
    >
      <Icon className="h-4 w-4 stroke-[1.75]" aria-hidden="true" />
    </button>
  </Tooltip>
);

/** A wrapping "a · b · c" line. Each item brings its own dot, and the dot
 *  that would start a line sits in a clipped margin, so no line starts or
 *  ends with one. */
const MetaLine: React.FC<{ children: React.ReactNode }> = ({ children }) => (
  <div className="mt-1.5 overflow-hidden text-[13px] text-text-3">
    <div className="-ml-[21px] flex flex-wrap items-center gap-y-1">
      {React.Children.toArray(children).map((item, i) => (
        <span key={i} className="inline-flex min-w-0 items-center">
          <span
            aria-hidden="true"
            className="w-[21px] flex-none text-center text-text-4"
          >
            ·
          </span>
          {item}
        </span>
      ))}
    </div>
  </div>
);

interface HistoryRowProps {
  entry: HistoryEntry;
  actions: RowActions;
  focused: boolean;
  menuOpen: boolean;
  setMenuOpen: (open: boolean) => void;
  onFocusRow: () => void;
  /** This row is in Fix a word mode. */
  fixing: boolean;
  onFix: () => void;
  onExitFix: (fixed: boolean) => void;
}

export const HistoryRow: React.FC<HistoryRowProps> = ({
  entry,
  actions,
  focused,
  menuOpen,
  setMenuOpen,
  onFocusRow,
  fixing,
  onFix,
  onExitFix,
}) => {
  const { t, i18n } = useTranslation();
  const [playing, setPlaying] = useState(false);
  const [copied, setCopied] = useState(false);
  const [retrying, setRetrying] = useState(false);
  const [wholeOriginal, setWholeOriginal] = useState(false);

  const text = finalText(entry);
  const failed = entry.status === "failed" || !entry.transcription_text.trim();
  // Dictations from a paired phone keep their audio on the phone.
  const hasAudio = !!entry.file_name;
  const time = new Date(entry.timestamp * 1000).toLocaleTimeString(
    i18n.language,
    { hour: "numeric", minute: "2-digit" },
  );

  const copy = async (value: string) => {
    if (await actions.copy(entry, value)) {
      setCopied(true);
      setTimeout(() => setCopied(false), 1200);
    }
  };

  const retry = async () => {
    setRetrying(true);
    await actions.retry(entry).finally(() => setRetrying(false));
  };

  const loadAudio = useCallback(
    () => actions.loadAudio(entry),
    [actions, entry],
  );

  const menuItems: ActionMenuItem[] = [
    ...(failed
      ? []
      : [
          {
            id: "fix",
            label: t("home.row.fixWord"),
            icon: SpellCheck,
            shortcut: "F",
            onSelect: onFix,
          },
        ]),
    ...(hasAudio
      ? [
          {
            id: "retry",
            label: t("home.row.transcribeAgain"),
            icon: RotateCcw,
            disabled: retrying,
            onSelect: retry,
          },
        ]
      : []),
    ...(entry.post_processed_text
      ? [
          {
            id: "copy-original",
            label: t("home.row.copyOriginal"),
            icon: Copy,
            onSelect: () => copy(entry.transcription_text),
          },
        ]
      : []),
    {
      id: "star",
      label: entry.saved ? t("home.row.unstar") : t("home.row.star"),
      icon: Star,
      shortcut: "S",
      onSelect: () => actions.toggleStar(entry),
    },
    ...(hasAudio
      ? [
          {
            id: "save-audio",
            label: t("home.row.saveAudio"),
            icon: Download,
            onSelect: () => actions.saveAudio(entry),
          },
        ]
      : []),
    "separator",
    {
      id: "delete",
      label: t("home.row.delete"),
      icon: Trash2,
      shortcut: t("home.row.deleteKey"),
      danger: true,
      onSelect: () => actions.remove(entry),
    },
  ];

  const appLabel = entry.app_name
    ? entry.app_site
      ? t("home.meta.siteIn", { site: entry.app_site, app: entry.app_name })
      : entry.app_name
    : null;

  return (
    <li
      data-row={entry.id}
      tabIndex={focused ? 0 : -1}
      aria-label={[time, appLabel].filter(Boolean).join(", ")}
      onFocus={(e) => e.target === e.currentTarget && onFocusRow()}
      onContextMenu={(e) => {
        e.preventDefault();
        onFocusRow();
        setMenuOpen(true);
      }}
      className={`group relative grid grid-cols-[68px_minmax(0,1fr)_32px] gap-x-3 rounded-[var(--r-md)] px-3 py-3 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0 focus-visible:bg-surface-2 ring-inside ${menuOpen ? "bg-surface-2" : ""}`}
    >
      <div className="flex flex-col gap-1.5 pt-0.5 text-[13px] tabular-nums text-text-3">
        <span className="whitespace-nowrap">{time}</span>
        {entry.saved && (
          <Star
            className="h-3.5 w-3.5 fill-teal-600 text-teal-600"
            aria-label={t("home.row.starred")}
          />
        )}
      </div>

      <div className="min-w-0">
        {retrying ? (
          <p className="animate-pulse text-[15px] leading-[1.55] text-text-3">
            {t("home.row.transcribing")}
          </p>
        ) : failed ? (
          <p className="text-[15px] leading-[1.55] text-text-3">
            {t("home.row.failed")}{" "}
            <button
              type="button"
              onClick={retry}
              className="font-medium text-teal-700 underline-offset-2 hover:underline"
            >
              {t("home.row.tryAgain")}
            </button>
          </p>
        ) : fixing ? (
          <FixWord entry={entry} onExit={onExitFix} />
        ) : (
          <p
            data-history-text
            className="cursor-text select-text whitespace-pre-wrap break-words text-[15px] leading-[1.55] text-text"
          >
            {markFixes(text, entry.fixes).map((seg, i) =>
              seg.fix ? (
                <span
                  key={i}
                  className="fixed-word"
                  title={t("home.row.fixedTip", {
                    word: seg.fix.to,
                    heard: seg.fix.from,
                    by: t(
                      seg.fix.source === "manual"
                        ? "home.row.fixedByYou"
                        : seg.fix.source === "ai"
                          ? "home.row.fixedByAi"
                          : "home.row.fixedByDictionary",
                    ),
                  })}
                >
                  {seg.text}
                </span>
              ) : (
                <React.Fragment key={i}>{seg.text}</React.Fragment>
              ),
            )}
          </p>
        )}

        {entry.post_processed_text != null &&
          !failed &&
          !fixing &&
          !retrying &&
          entry.transcription_text.trim() !== text.trim() && (
            <div className="mt-2 border-s-2 border-line ps-3 text-[13px] leading-[1.5] text-text-3">
              <p
                className={`select-text whitespace-pre-wrap break-words ${wholeOriginal ? "" : "line-clamp-2"}`}
              >
                <span className="font-medium text-text-2">
                  {entry.transform
                    ? t("home.row.before", { name: entry.transform })
                    : t("home.row.beforeTransform")}
                </span>{" "}
                {entry.transcription_text}
              </p>
              {entry.transcription_text.length > 180 && (
                <button
                  type="button"
                  onClick={() => setWholeOriginal((w) => !w)}
                  className="mt-0.5 block font-medium text-teal-700 underline-offset-2 hover:underline"
                >
                  {wholeOriginal
                    ? t("home.row.showLess")
                    : t("home.row.showAll")}
                </button>
              )}
            </div>
          )}

        <MetaLine>
          {appLabel && (
            <span
              className="inline-flex min-w-0 items-center gap-1.5 text-text-2"
              title={entry.app_title ?? undefined}
            >
              <AppIcon path={entry.app_path} />
              <span data-history-app className="truncate">
                {appLabel}
              </span>
            </span>
          )}
          {entry.device && (
            <span className="inline-flex items-center gap-1 text-text-2">
              <Smartphone
                className="h-3 w-3 flex-none stroke-[1.75]"
                aria-hidden="true"
              />
              {t("home.meta.onDevice", { device: entry.device })}
            </span>
          )}
          {entry.model_name && <span>{entry.model_name}</span>}
          {entry.speech_ms != null && entry.speech_ms > 0 && (
            <span>{formatSpeech(entry.speech_ms)}</span>
          )}
          {entry.status === "pasted" && (
            <span>
              {entry.paste_app_name
                ? t("home.meta.pastedInto", { app: entry.paste_app_name })
                : entry.paste_ms != null
                  ? t("home.meta.pastedIn", {
                      time: formatSpeech(entry.paste_ms),
                    })
                  : t("home.meta.pasted")}
            </span>
          )}
          {entry.status === "not_pasted" && !entry.device && (
            <span>{t("home.meta.notPasted")}</span>
          )}
          {entry.transform && (
            <span>{t("home.meta.transformed", { name: entry.transform })}</span>
          )}
          {entry.fixes.length > 0 && (
            <span className="text-teal-700">
              {t("home.meta.fixes", { count: entry.fixes.length })}
            </span>
          )}
        </MetaLine>

        {entry.status === "paste_failed" && (
          <p className="mt-1 text-[13px] text-danger">
            {t("home.meta.pasteFailed", {
              reason: entry.error ?? t("home.meta.unknownReason"),
            })}
          </p>
        )}

        {playing && (
          <AudioPlayer
            onLoadRequest={loadAudio}
            autoPlay
            className="mt-2.5 max-w-[420px]"
          />
        )}
      </div>

      {/* The actions stand in a column in the row's right gutter; on a
          short row it reaches over the gutter of the row below. */}
      <div
        className={`absolute end-2 top-2 z-10 flex flex-col gap-0.5 rounded-[var(--r-md)] border border-line bg-surface p-0.5 shadow-[var(--shadow-1)] transition-opacity duration-[var(--dur-1)] group-hover:opacity-100 group-hover:duration-0 group-focus-within:pointer-events-auto group-focus-within:opacity-100 group-hover:pointer-events-auto ${menuOpen || playing || fixing ? "opacity-100" : "pointer-events-none opacity-0"}`}
      >
        {hasAudio && (
          <RowButton
            label={playing ? t("home.row.hidePlayer") : t("home.row.play")}
            icon={playing ? Pause : Play}
            pressed={playing}
            onClick={() => setPlaying((p) => !p)}
          />
        )}
        {!failed && (
          <RowButton
            label={copied ? t("home.row.copied") : t("home.row.copy")}
            icon={copied ? Check : Copy}
            shortcut="Enter"
            onClick={() => copy(text)}
          />
        )}
        {!failed && (
          <RowButton
            label={t("home.row.fixWord")}
            icon={SpellCheck}
            shortcut="F"
            pressed={fixing}
            onClick={fixing ? () => onExitFix(false) : onFix}
          />
        )}
        <RowButton
          label={t("home.row.more")}
          icon={EllipsisVertical}
          expanded={menuOpen}
          onClick={() => setMenuOpen(!menuOpen)}
        />
        {menuOpen && (
          <ActionMenu
            items={menuItems}
            label={t("home.row.more")}
            onClose={(restore) => {
              setMenuOpen(false);
              if (restore)
                document
                  .querySelector<HTMLElement>(`[data-row="${entry.id}"]`)
                  ?.focus();
            }}
          />
        )}
      </div>
    </li>
  );
};
