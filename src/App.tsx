import {
  useEffect,
  useLayoutEffect,
  useState,
  useRef,
  type ReactNode,
} from "react";
import { toast } from "sonner";
import { useTranslation } from "react-i18next";
import { listen } from "@tauri-apps/api/event";
import { ModelStateEvent, RecordingErrorEvent } from "./lib/types/events";
import "./App.css";
import Onboarding, {
  AccessibilityOnboarding,
  ModelMigration,
} from "./components/onboarding";
import {
  ModelsSettings,
  type OnboardingPreviewStep,
} from "./components/settings";
import { SettingsPage } from "./components/settings/SettingsPage";
import { DictionaryPage } from "./components/pages/dictionary/DictionaryPage";
import { SnippetsPage } from "./components/pages/snippets/SnippetsPage";
import { ScratchpadPage } from "./components/pages/scratchpad/ScratchpadPage";
import { StylePage } from "./components/pages/style/StylePage";
import { TransformsPage } from "./components/pages/transforms/TransformsPage";
import { NotesPage } from "./components/pages/notes/NotesPage";
import { InsightsPage } from "./components/pages/insights/InsightsPage";
import { watchNotes } from "./stores/notesStore";
import { watchCalls } from "./stores/callsStore";
import { ErrorBoundary } from "./components/ErrorBoundary";
import { Sidebar } from "./components/Sidebar";
import { AppToaster } from "./components/shell/AppToaster";
import { TitleBar } from "./components/shell/TitleBar";
import { WhatsNewGate } from "./components/whats-new";
import {
  useNavStore,
  type Page,
  type SettingsSection,
} from "./stores/navStore";
import { useSettings } from "./hooks/useSettings";
import { useSettingsStore } from "./stores/settingsStore";
import { commands, events } from "@/bindings";
import { HomePage } from "@/components/pages/home/HomePage";
import { deleteRetiredModel } from "@/components/onboarding/retiredModels";
import { SetupFlow } from "@/components/setup/SetupFlow";
import { isStep, type Step } from "@/components/setup/setup";

type OnboardingStep =
  | "accessibility"
  | "migration"
  | "model"
  | "setup"
  | "done";

// Stable identity so preview effects do not re-run due to callback changes.
const NOOP = () => {};

const renderPage = (
  page: Page,
  onPreviewOnboarding: (step: OnboardingPreviewStep) => void,
) => {
  switch (page) {
    case "dictionary":
      return <DictionaryPage />;
    case "snippets":
      return <SnippetsPage />;
    case "style":
      return <StylePage />;
    case "scratchpad":
      return <ScratchpadPage />;
    case "transforms":
      return <TransformsPage />;
    case "notes":
      return <NotesPage />;
    case "insights":
      return <InsightsPage />;
    case "models":
      return <ModelsSettings />;
    case "settings":
      return <SettingsPage onPreviewOnboarding={onPreviewOnboarding} />;
    default:
      return <HomePage />;
  }
};

function App() {
  const { t } = useTranslation();
  const [onboardingStep, setOnboardingStep] = useState<OnboardingStep | null>(
    null,
  );
  const [onboardingPreview, setOnboardingPreview] =
    useState<OnboardingPreviewStep | null>(null);
  // Track if this is a returning user who just needs to grant permissions
  // (vs a new user who needs full onboarding including model selection)
  const [isReturningUser, setIsReturningUser] = useState(false);
  // Set when a returning user's selected model is a retired (ONNX) model that
  // is still on disk: they must update it before the app can transcribe.
  const [migrationModelId, setMigrationModelId] = useState<string | null>(null);
  // The setup step to open (new users, or setup left part-way).
  const [setupStep, setSetupStep] = useState<Step>("mic");
  const page = useNavStore((state) => state.page);
  const { settings, updateSetting } = useSettings();
  const refreshAudioDevices = useSettingsStore(
    (state) => state.refreshAudioDevices,
  );
  const refreshOutputDevices = useSettingsStore(
    (state) => state.refreshOutputDevices,
  );
  const hasCompletedPostOnboardingInit = useRef(false);
  const settingsScrollRef = useRef<HTMLDivElement>(null);
  const isShowingOnboarding =
    onboardingPreview !== null ||
    onboardingStep === "accessibility" ||
    onboardingStep === "migration" ||
    onboardingStep === "model" ||
    onboardingStep === "setup";

  // The scrollbar consumes layout space. Reserve a matching gutter on the
  // opposite edge while onboarding is visible so its content stays centered in
  // the physical window.
  useLayoutEffect(() => {
    const attribute = "data-onboarding-active";
    document.documentElement.toggleAttribute(attribute, isShowingOnboarding);
    return () => document.documentElement.removeAttribute(attribute);
  }, [isShowingOnboarding]);

  // Reset the scroll position whenever the page changes.
  useLayoutEffect(() => {
    settingsScrollRef.current?.scrollTo({ top: 0 });
  }, [page]);

  useEffect(() => {
    checkOnboardingStatus();
  }, []);

  // "Back to setup" on the toast after skipping it.
  useEffect(() => {
    const onSetup = (e: Event) => {
      const step = (e as CustomEvent<{ step: string }>).detail?.step;
      setSetupStep(isStep(step) ? step : "mic");
      setOnboardingStep("setup");
    };
    window.addEventListener("legato:setup", onSetup);
    return () => window.removeEventListener("legato:setup", onSetup);
  }, []);

  // Initialize Enigo, shortcuts, and refresh audio devices when main app loads
  useEffect(() => {
    if (onboardingStep === "done" && !hasCompletedPostOnboardingInit.current) {
      hasCompletedPostOnboardingInit.current = true;
      Promise.all([
        commands.initializeEnigo(),
        commands.initializeShortcuts(),
      ]).catch((e) => {
        console.warn("Failed to initialize:", e);
      });
      refreshAudioDevices();
      refreshOutputDevices();
    }
  }, [onboardingStep, refreshAudioDevices, refreshOutputDevices]);

  // Handle keyboard shortcuts for debug mode toggle
  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      // Ctrl+Shift+D (or Win+Shift+D) toggles debug mode
      const isDebugShortcut =
        event.shiftKey &&
        event.key.toLowerCase() === "d" &&
        (event.ctrlKey || event.metaKey);

      if (isDebugShortcut) {
        event.preventDefault();
        const currentDebugMode = settings?.debug_mode ?? false;
        updateSetting("debug_mode", !currentDebugMode);
      }
    };

    // Add event listener when component mounts
    document.addEventListener("keydown", handleKeyDown);

    // Cleanup event listener when component unmounts
    return () => {
      document.removeEventListener("keydown", handleKeyDown);
    };
  }, [settings?.debug_mode, updateSetting]);

  // Listen for recording errors from the backend and show a toast
  useEffect(() => {
    const unlisten = listen<RecordingErrorEvent>("recording-error", (event) => {
      const { error_type, detail } = event.payload;

      if (error_type === "microphone_permission_denied") {
        toast.error(t("errors.micPermissionDeniedTitle"), {
          description: t("errors.micPermissionDenied"),
        });
      } else if (error_type === "no_input_device") {
        toast.error(t("errors.noInputDeviceTitle"), {
          description: t("errors.noInputDevice"),
        });
      } else {
        toast.error(
          t("errors.recordingFailed", { error: detail ?? "Unknown error" }),
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for paste failures and show a toast.
  // The technical error detail is logged to legato.log on the Rust side
  // (see actions.rs `error!("Failed to paste transcription: ...")`),
  // so we show a localized, user-friendly message here instead of the raw error.
  // Notes: the recording's state, and words dictated into My notes, arrive
  // on any page.
  useEffect(watchNotes, []);
  // A test call card's buttons answer with a toast, from any page.
  useEffect(watchCalls, []);

  // The Legato bar's Review and other windows ask for a page.
  useEffect(() => {
    const unlisten = events.openPage.listen((event) => {
      const { page, section } = event.payload;
      const goTo = useNavStore.getState().goTo;
      if (page === "settings") {
        useNavStore
          .getState()
          .go("settings", (section ?? undefined) as SettingsSection);
      } else {
        goTo(page as Page, section);
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, []);

  useEffect(() => {
    const unlisten = listen("paste-error", () => {
      toast.error(t("errors.pasteFailedTitle"), {
        description: t("errors.pasteFailed"),
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for transcription failures and show a toast.
  // The payload is the backend error message (also logged to legato.log).
  useEffect(() => {
    const unlisten = listen<string>("transcription-error", (event) => {
      toast.error(t("errors.transcriptionFailedTitle"), {
        description: event.payload,
      });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  // Listen for model loading failures and show a toast
  useEffect(() => {
    const unlisten = listen<ModelStateEvent>("model-state-changed", (event) => {
      if (event.payload.event_type === "loading_failed") {
        toast.error(
          t("errors.modelLoadFailed", {
            model:
              event.payload.model_name || t("errors.modelLoadFailedUnknown"),
          }),
          {
            description: event.payload.error,
          },
        );
      }
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [t]);

  const revealMainWindowForPermissions = async () => {
    try {
      await commands.showMainWindowCommand();
    } catch (e) {
      console.warn("Failed to show main window for permission onboarding:", e);
    }
  };

  const getMigrationModelId = async (
    selectedModel: string,
  ): Promise<string | null> => {
    const retired = await commands.getRetiredModels();
    return retired.some((m) => m.id === selectedModel) ? selectedModel : null;
  };

  // Returning users land in the app unless their model needs updating. The
  // window may start hidden, so bring it forward for the update screen.
  const finishReturningUser = async (retiredModelId: string | null) => {
    if (retiredModelId) {
      await revealMainWindowForPermissions();
      setOnboardingStep("migration");
    } else {
      setOnboardingStep("done");
    }
  };

  const checkOnboardingStatus = async () => {
    try {
      const settingsResult = await commands.getAppSettings();
      const hasCompletedOnboarding =
        settingsResult.status === "ok" &&
        settingsResult.data.onboarding_completed === true;

      // Setup left part-way (picking a model already marks onboarding done)
      // comes back to the same step.
      const resume =
        settingsResult.status === "ok" ? settingsResult.data.setup_step : "";
      if (isStep(resume)) {
        await revealMainWindowForPermissions();
        setSetupStep(resume);
        setOnboardingStep("setup");
        return;
      }

      if (hasCompletedOnboarding) {
        // Returning user - check if they need to grant permissions first
        setIsReturningUser(true);
        const retiredModelId = await getMigrationModelId(
          settingsResult.data.selected_model ?? "",
        );
        setMigrationModelId(retiredModelId);

        try {
          const microphoneStatus =
            await commands.getWindowsMicrophonePermissionStatus();
          if (microphoneStatus.overall_access === "denied") {
            await revealMainWindowForPermissions();
            setOnboardingStep("accessibility");
            return;
          }
        } catch (e) {
          console.warn("Failed to check Windows microphone permissions:", e);
          // If we can't check, proceed to main app and let them fix it there
        }

        await finishReturningUser(retiredModelId);
      } else {
        // New user: setup, which checks microphone access itself.
        setIsReturningUser(false);
        setSetupStep("mic");
        setOnboardingStep("setup");
      }
    } catch (error) {
      console.error("Failed to check onboarding status:", error);
      setOnboardingStep("accessibility");
    }
  };

  const handleAccessibilityComplete = () => {
    // Returning users already have models, skip to main app (unless their
    // model needs updating). New users need to select a model.
    if (isReturningUser) {
      finishReturningUser(migrationModelId);
    } else {
      setOnboardingStep("model");
    }
  };

  const handleModelSelected = async () => {
    // Coming from the model update: the user picked a different model instead
    // of updating, so the retired models left on disk can't run anymore.
    if (migrationModelId) {
      for (const model of await commands.getRetiredModels()) {
        await deleteRetiredModel(model.id);
      }
    }
    // Transition to main app - user has started a download
    setOnboardingStep("done");
  };

  // Rendered once around every step below (including onboarding) so
  // toast.error() calls surface to the user. sonner renders via a portal, so
  // its position in the tree doesn't affect layout. Without this, errors during
  // onboarding (e.g. a model download failing because blob.handy.computer is
  // unreachable) are silently swallowed and the wizard just appears to "blink".
  const toaster = <AppToaster />;

  // Still checking onboarding status
  if (onboardingStep === null) {
    return null;
  }

  // Select the content for the current step. The Toaster is rendered once, in a
  // stable wrapper around this node, so crossing between onboarding steps and
  // the main app never remounts it (which would drop any in-flight toast).
  let content: ReactNode;
  let inShell = false;
  if (onboardingPreview) {
    // Render previews in the same top-level slot as real onboarding. Keeping
    // the settings layout unmounted ensures viewport overflow behaves exactly
    // as it does during first-run onboarding.
    content = (
      <>
        {onboardingPreview === "accessibility" ? (
          <AccessibilityOnboarding onComplete={NOOP} preview />
        ) : (
          <Onboarding onModelSelected={NOOP} preview />
        )}
        <button
          type="button"
          onClick={() => setOnboardingPreview(null)}
          className="fixed top-16 end-4 z-50 rounded-lg border border-line-strong bg-surface px-4 py-2 text-sm font-medium text-text shadow-lg hover:bg-surface-2 cursor-pointer"
        >
          {t("settings.debug.onboardingPreview.exitButton")}
        </button>
      </>
    );
  } else if (onboardingStep === "setup") {
    content = (
      <SetupFlow
        initialStep={setupStep}
        onDone={(target, sub) => {
          setOnboardingStep("done");
          useNavStore.getState().go(target, sub as SettingsSection | undefined);
        }}
      />
    );
  } else if (onboardingStep === "accessibility") {
    content = (
      <AccessibilityOnboarding onComplete={handleAccessibilityComplete} />
    );
  } else if (onboardingStep === "migration" && migrationModelId) {
    content = (
      <ModelMigration
        inUseModelId={migrationModelId}
        onComplete={() => setOnboardingStep("done")}
        onChooseAnother={() => setOnboardingStep("model")}
      />
    );
  } else if (onboardingStep === "model") {
    content = (
      <Onboarding
        onModelSelected={handleModelSelected}
        onBack={
          migrationModelId ? () => setOnboardingStep("migration") : undefined
        }
      />
    );
  } else {
    inShell = true;
    content = (
      <div className="flex h-screen cursor-default select-none flex-col bg-bg">
        <ErrorBoundary context="What's New">
          <WhatsNewGate />
        </ErrorBoundary>
        <TitleBar />
        <div className="grid min-h-0 flex-1 grid-cols-[212px_minmax(0,1fr)]">
          <Sidebar />
          <main className="mb-3.5 me-3.5 min-h-0 overflow-hidden rounded-[var(--r-xl)] border border-line bg-surface">
            <div
              ref={settingsScrollRef}
              data-scroll-root
              className="h-full overflow-y-auto px-8 pt-8 pb-14 min-[1180px]:px-16 min-[1180px]:pt-10 [scrollbar-gutter:stable]"
            >
              <ErrorBoundary key={page} context={page}>
                <div key={page} className="view-enter">
                  {renderPage(page, setOnboardingPreview)}
                </div>
              </ErrorBoundary>
            </div>
          </main>
        </div>
      </div>
    );
  }

  // Onboarding and its previews fill the window under the title bar, which
  // only has the window controls there.
  if (!inShell) {
    content = (
      <div className="flex h-screen flex-col bg-bg">
        <TitleBar minimal />
        <div className="min-h-0 flex-1 overflow-y-auto">{content}</div>
      </div>
    );
  }

  return (
    <>
      {toaster}
      {content}
    </>
  );
}

export default App;
