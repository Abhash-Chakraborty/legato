import { useEffect, useState, useCallback, useRef } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { commands } from "@/bindings";
import { useSettingsStore } from "@/stores/settingsStore";
import { isMac } from "@/lib/utils/platform";
import LegatoLogo from "../icons/LegatoLogo";
import { Keyboard, Mic, Check, Loader2 } from "@/icons";

interface AccessibilityOnboardingProps {
  onComplete: () => void;
  preview?: boolean;
}

type PermissionStatus = "checking" | "needed" | "waiting" | "granted";

const AccessibilityOnboarding: React.FC<AccessibilityOnboardingProps> = ({
  onComplete,
  preview = false,
}) => {
  const { t } = useTranslation();
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const [microphone, setMicrophone] = useState<PermissionStatus>("checking");
  // A Mac also asks before Legato may hear its shortcut in other apps and
  // paste (Accessibility). Windows never asks, so there it's always granted.
  const [keyboard, setKeyboard] = useState<PermissionStatus>(
    isMac ? "checking" : "granted",
  );
  const pollingRef = useRef<ReturnType<typeof setInterval> | null>(null);
  const timeoutRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const errorCountRef = useRef<number>(0);
  const MAX_POLLING_ERRORS = 3;

  const completeOnboarding = useCallback(async () => {
    await Promise.all([refreshAudioDevices(), refreshOutputDevices()]);
    timeoutRef.current = setTimeout(() => onComplete(), 300);
  }, [onComplete, refreshAudioDevices, refreshOutputDevices]);

  const hasWindowsMicrophoneAccess = useCallback(async (): Promise<boolean> => {
    const microphoneStatus =
      await commands.getWindowsMicrophonePermissionStatus();

    return microphoneStatus.overall_access !== "denied";
  }, []);

  const hasKeyboardAccess = useCallback(
    async (): Promise<boolean> =>
      !isMac || (await commands.keyboardAccessGranted()),
    [],
  );

  // Check permission status on mount
  useEffect(() => {
    // Debug previews are intentionally inert: show the permission request UI
    // without checking or changing operating-system permissions.
    if (preview) {
      setMicrophone("needed");
      if (isMac) setKeyboard("needed");
      return;
    }

    const checkInitial = async () => {
      try {
        const microphoneGranted = await hasWindowsMicrophoneAccess();
        const keyboardGranted = await hasKeyboardAccess();

        setMicrophone(microphoneGranted ? "granted" : "needed");
        setKeyboard(keyboardGranted ? "granted" : "needed");

        if (microphoneGranted && keyboardGranted) {
          await completeOnboarding();
        }
      } catch (error) {
        console.warn("Failed to check Windows microphone permissions:", error);
        setMicrophone("granted");
        setKeyboard("granted");
        await completeOnboarding();
      }
    };

    checkInitial();
  }, [
    completeOnboarding,
    hasWindowsMicrophoneAccess,
    hasKeyboardAccess,
    preview,
  ]);

  // Polling for permissions after user clicks a button
  const startPolling = useCallback(() => {
    if (pollingRef.current) return;

    pollingRef.current = setInterval(async () => {
      try {
        const microphoneGranted = await hasWindowsMicrophoneAccess();
        const keyboardGranted = await hasKeyboardAccess();

        if (microphoneGranted) setMicrophone("granted");
        if (keyboardGranted) setKeyboard("granted");

        if (microphoneGranted && keyboardGranted) {
          if (pollingRef.current) {
            clearInterval(pollingRef.current);
            pollingRef.current = null;
          }

          await completeOnboarding();
        }

        // Reset error count on success
        errorCountRef.current = 0;
      } catch (error) {
        console.error("Error checking permissions:", error);
        errorCountRef.current += 1;

        if (errorCountRef.current >= MAX_POLLING_ERRORS) {
          // Stop polling after too many consecutive errors
          if (pollingRef.current) {
            clearInterval(pollingRef.current);
            pollingRef.current = null;
          }
          toast.error(t("onboarding.permissions.errors.checkFailed"));
        }
      }
    }, 1000);
  }, [completeOnboarding, hasWindowsMicrophoneAccess, hasKeyboardAccess, t]);

  // Cleanup polling and timeouts on unmount
  useEffect(() => {
    return () => {
      if (pollingRef.current) {
        clearInterval(pollingRef.current);
      }
      if (timeoutRef.current) {
        clearTimeout(timeoutRef.current);
      }
    };
  }, []);

  const handleGrantMicrophone = async () => {
    if (preview) return;

    try {
      await commands.openMicrophonePrivacySettings();
      setMicrophone("waiting");
      startPolling();
    } catch (error) {
      console.error("Failed to request microphone permission:", error);
      toast.error(t("onboarding.permissions.errors.requestFailed"));
    }
  };

  const handleGrantKeyboard = async () => {
    if (preview) return;

    const asked = await commands.requestKeyboardAccess();
    if (asked.status === "error") {
      console.error("Failed to ask for Accessibility access:", asked.error);
      toast.error(t("onboarding.permissions.errors.requestFailed"));
      return;
    }
    setKeyboard("waiting");
    startPolling();
  };

  // Still checking initial permissions
  if (microphone === "checking" || keyboard === "checking") {
    return (
      <div className="h-full w-full flex items-center justify-center">
        <Loader2 className="w-8 h-8 animate-spin text-text/50" />
      </div>
    );
  }

  // All permissions granted - show success briefly
  if (microphone === "granted" && keyboard === "granted") {
    return (
      <div className="h-full w-full flex flex-col items-center justify-center gap-4">
        <div className="p-4 rounded-full bg-ok-soft">
          <Check className="w-12 h-12 text-ok" />
        </div>
        <p className="text-lg font-medium text-text">
          {t("onboarding.permissions.allGranted")}
        </p>
      </div>
    );
  }

  const cards = [
    {
      id: "microphone",
      status: microphone,
      icon: Mic,
      title: t("onboarding.permissions.microphone.title"),
      description: t("onboarding.permissions.microphone.description"),
      onGrant: handleGrantMicrophone,
    },
    {
      id: "keyboard",
      status: keyboard,
      icon: Keyboard,
      title: t("onboarding.permissions.keyboard.title"),
      description: t("onboarding.permissions.keyboard.description"),
      onGrant: handleGrantKeyboard,
    },
  ].filter((card) => card.status !== "granted");

  // Show permissions request screen
  return (
    <div className="h-full w-full flex flex-col p-6 gap-6 items-center justify-center">
      <div className="flex flex-col items-center gap-2">
        <LegatoLogo width={200} />
      </div>

      <div className="max-w-md w-full flex flex-col items-center gap-4">
        <div className="text-center mb-2">
          <h2 className="text-xl font-semibold text-text mb-2">
            {t("onboarding.permissions.title")}
          </h2>
          <p className="text-text/70">
            {t("onboarding.permissions.description")}
          </p>
        </div>

        {cards.map(
          ({ id, status, icon: Icon, title, description, onGrant }) => (
            <div
              key={id}
              className="w-full p-4 rounded-lg bg-white/5 border border-mid-gray/20"
            >
              <div className="flex items-center gap-4">
                <div className="p-3 rounded-full bg-logo-primary/20 shrink-0">
                  <Icon className="w-6 h-6 text-logo-primary" />
                </div>
                <div className="flex-1 min-w-0">
                  <h3 className="font-medium text-text">{title}</h3>
                  <p className="text-sm text-text/60 mb-3">{description}</p>
                  {status === "waiting" ? (
                    <div className="flex items-center gap-2 text-text/50 text-sm">
                      <Loader2 className="w-4 h-4 animate-spin" />
                      {t("onboarding.permissions.waiting")}
                    </div>
                  ) : (
                    <button
                      onClick={onGrant}
                      className="px-4 py-2 rounded-lg bg-logo-primary hover:bg-logo-primary/90 text-white text-sm font-medium transition-colors"
                    >
                      {t("accessibility.openSettings")}
                    </button>
                  )}
                </div>
              </div>
            </div>
          ),
        )}
      </div>
    </div>
  );
};

export default AccessibilityOnboarding;
