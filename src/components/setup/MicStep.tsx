import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  Check,
  ExternalLink,
  Mic,
  MicOff,
  ShieldAlert,
  ShieldCheck,
  TriangleAlert,
} from "lucide-react";
import { commands, events } from "@/bindings";
import { useSettings } from "../../hooks/useSettings";
import { Button } from "../ui/Button";
import { Dropdown } from "../ui/Dropdown";
import type { StepProps } from "./SetupFlow";
import { Panel, Row } from "./parts";

const BARS = 48;
/** Speech reads well above room noise on this scale (-60..0 dBFS). */
const VOICE = 0.42;
/** About 1.2 s of voice, at roughly 20 levels a second. */
const HEARD_AFTER = 24;
/** No level at all for this long means the microphone didn't open. */
const SILENT_MS = 4000;

type Access = "checking" | "allowed" | "blocked";
type CheckState = "listening" | "heard" | "failed";

/** Microphone access, the microphone to use, and a level check that turns
 *  on Continue once Legato hears a sentence or two. */
export const MicStep: React.FC<StepProps> = ({ go, setFoot }) => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, audioDevices, refreshAudioDevices } =
    useSettings();
  const [access, setAccess] = useState<Access>("checking");
  const [opened, setOpened] = useState(false);
  const [still, setStill] = useState(false);
  const [check, setCheck] = useState<CheckState>("listening");
  const [levels, setLevels] = useState<number[]>(() => Array(BARS).fill(0));
  const voiced = useRef(0);
  const lastLevel = useRef(0);

  const selected = getSetting("selected_microphone") || "Default";
  const microphone = selected === "default" ? "Default" : selected;

  const readAccess = async () => {
    try {
      const status = await commands.getWindowsMicrophonePermissionStatus();
      return status.overall_access === "denied" ? "blocked" : "allowed";
    } catch {
      // Can't tell: the level check below says whether sound comes through.
      return "allowed";
    }
  };

  useEffect(() => {
    refreshAudioDevices();
    readAccess().then(setAccess);
  }, []);

  // The level check runs while access is allowed, and starts again for a
  // newly chosen microphone.
  useEffect(() => {
    if (access !== "allowed") return;
    voiced.current = 0;
    lastLevel.current = Date.now();
    setCheck("listening");
    setLevels(Array(BARS).fill(0));
    commands.startMicCheck();
    const off = events.micCheckLevel.listen(({ payload }) => {
      lastLevel.current = Date.now();
      setLevels((prev) => [...prev.slice(1), payload.level]);
      if (payload.level > VOICE) voiced.current += 1;
      if (voiced.current >= HEARD_AFTER)
        setCheck((c) => (c === "listening" ? "heard" : c));
    });
    const silent = setInterval(() => {
      if (Date.now() - lastLevel.current > SILENT_MS)
        setCheck((c) => (c === "listening" ? "failed" : c));
    }, 500);
    return () => {
      clearInterval(silent);
      off.then((fn) => fn());
      commands.stopMicCheck();
    };
  }, [access, microphone]);

  useEffect(() => {
    setFoot({
      back: false,
      disabled: access !== "allowed" || check === "listening",
      note:
        access === "blocked"
          ? t("setup.mic.noteBlocked")
          : check === "listening"
            ? t("setup.mic.noteWaiting")
            : check === "failed"
              ? t("setup.mic.noteFailed")
              : "",
      onNext: () => go("model"),
    });
  }, [access, check, go, setFoot, t]);

  const checkAgain = async () => {
    setStill(false);
    const now = await readAccess();
    if (now === "blocked") setStill(true);
    setAccess(now);
  };

  const options = audioDevices.map((d) => ({ value: d.name, label: d.name }));
  const current = levels[BARS - 1] ?? 0;

  const hear =
    access === "blocked"
      ? { icon: MicOff, text: t("setup.mic.hearBlocked"), sub: "" }
      : check === "heard"
        ? {
            icon: Check,
            text: t("setup.mic.hearOk"),
            sub: t("setup.mic.hearOkSub"),
          }
        : check === "failed"
          ? {
              icon: TriangleAlert,
              text: t("setup.mic.hearFailed"),
              sub: t("setup.mic.hearFailedSub"),
            }
          : {
              icon: Mic,
              text: t("setup.mic.hearWait"),
              sub: t("setup.mic.hearWaitSub"),
            };
  const HearIcon = hear.icon;

  return (
    <Panel
      title={t("setup.mic.title")}
      sub={t("setup.mic.sub")}
      left={
        <div className="mt-6 overflow-hidden rounded-[var(--r-lg)] border border-line">
          {access === "blocked" ? (
            <Row
              icon={<ShieldAlert className="h-4 w-4" />}
              tone="warn"
              title={t("setup.mic.blocked")}
              help={
                still
                  ? t("setup.mic.stillBlocked")
                  : opened
                    ? t("setup.mic.blockedAfterOpen")
                    : t("setup.mic.blockedHelp")
              }
            >
              <div className="mt-3 flex flex-wrap gap-2">
                <Button
                  variant={opened ? "ghost" : "secondary"}
                  size="sm"
                  onClick={() => {
                    commands.openMicrophonePrivacySettings();
                    setOpened(true);
                  }}
                >
                  <ExternalLink className="h-3.5 w-3.5" aria-hidden="true" />
                  {t("setup.mic.openPrivacy")}
                </Button>
                <Button
                  variant={opened ? "secondary" : "ghost"}
                  size="sm"
                  onClick={checkAgain}
                >
                  {t("setup.mic.checkAgain")}
                </Button>
              </div>
            </Row>
          ) : (
            <Row
              icon={<ShieldCheck className="h-4 w-4" />}
              tone="ok"
              title={t("setup.mic.access")}
              help={t("setup.mic.accessHelp")}
              aside={
                access === "allowed" && (
                  <span className="flex items-center gap-1.5 text-[13px] text-text-2">
                    <span className="h-1.5 w-1.5 rounded-full bg-teal-500" />
                    {t("setup.mic.allowed")}
                  </span>
                )
              }
            />
          )}
          <Row
            icon={<Mic className="h-4 w-4" />}
            title={t("setup.mic.microphone")}
            help={
              microphone === "Default"
                ? t("setup.mic.followsDefault")
                : t("setup.mic.chosen")
            }
            disabled={access === "blocked"}
          >
            <Dropdown
              className="mt-3 w-full"
              options={options}
              selectedValue={microphone}
              onSelect={(v) => updateSetting("selected_microphone", v)}
              onOpen={refreshAudioDevices}
              disabled={access === "blocked" || options.length === 0}
              ariaLabel={t("setup.mic.microphone")}
            />
          </Row>
        </div>
      }
      right={
        <div className="rounded-[var(--r-lg)] border border-line p-5">
          <div className="flex items-baseline justify-between gap-3">
            <b className="text-sm font-medium">{t("setup.mic.level")}</b>
            <span className="truncate text-xs text-text-3">
              {microphone === "Default" ? t("setup.mic.default") : microphone}
            </span>
          </div>
          <div
            role="meter"
            aria-label={t("setup.mic.level")}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(current * 100)}
            className="mt-4 flex h-32 items-center justify-center gap-[3px] rounded-[var(--r-md)] bg-surface-2 px-5"
          >
            {levels.map((v, i) => (
              <i
                key={i}
                className={`block w-[5px] flex-none rounded-full transition-transform duration-75 ${
                  access === "blocked"
                    ? "bg-line-strong"
                    : v > VOICE
                      ? "bg-teal-600"
                      : "bg-teal-300"
                }`}
                style={{
                  height: "80%",
                  transform: `scaleY(${Math.max(0.05, v).toFixed(3)})`,
                }}
              />
            ))}
          </div>
          <div
            aria-live="polite"
            className={`mt-4 flex items-center gap-2 text-sm font-medium ${
              check === "failed" ? "text-warn" : ""
            }`}
          >
            <HearIcon
              className={`h-4 w-4 flex-none ${check === "heard" ? "text-teal-600" : ""}`}
              aria-hidden="true"
            />
            {hear.text}
          </div>
          {hear.sub && (
            <p className="mt-1 ps-6 text-[13px] text-text-3">{hear.sub}</p>
          )}
        </div>
      }
    />
  );
};
