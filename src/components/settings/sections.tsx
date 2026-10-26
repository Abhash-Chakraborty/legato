import React from "react";
import {
  Blocks,
  Bug,
  CalendarDays,
  Captions,
  ClipboardPaste,
  Info,
  Keyboard,
  Mic,
  PanelBottom,
  PhoneIncoming,
  Plug,
  Settings2,
  Shield,
  Smartphone,
  type LucideIcon,
} from "lucide-react";
import type { AiStatus, AppSettings, ModelInfo } from "@/bindings";
import type { SettingsSection } from "../../stores/navStore";
import { useModelStore } from "../../stores/modelStore";

import { AdminAppsToggle } from "./AdminAppsToggle";
import { YourName } from "./YourName";
import { FeatureToggle } from "./FeatureSettings";
import { InyonAlertsRow, InyonCard } from "./connections/InyonSettings";
import { NotesFolderRow } from "./connections/NotesFolder";
import { AutostartToggle } from "./AutostartToggle";
import { ShowTrayIcon } from "./ShowTrayIcon";
import { StartHidden } from "./StartHidden";
import { ThemeSelector } from "./ThemeSelector";
import { ShortcutInput } from "./ShortcutInput";
import { ShortcutActivationSetting } from "./ShortcutActivation";
import { HoldThreshold } from "./debug/HoldThreshold";
import { KeyboardImplementationSelector } from "./debug/KeyboardImplementationSelector";
import { MicrophoneSelector } from "./MicrophoneSelector";
import { MicrophoneOrder } from "./MicrophoneOrder";
import { ChannelSelector } from "./ChannelSelector";
import { BackgroundAudio } from "./BackgroundAudio";
import { LazyStreamClose } from "./LazyStreamClose";
import { AudioFeedback } from "./AudioFeedback";
import { SoundPicker } from "./SoundPicker";
import { VolumeSlider } from "./VolumeSlider";
import { OutputDeviceSelector } from "./OutputDeviceSelector";
import { LanguageSelector } from "./LanguageSelector";
import { TranslateToEnglish } from "./TranslateToEnglish";
import { ChineseScriptSetting } from "./ChineseScript";
import { VoiceActivityDetection } from "./VoiceActivityDetection";
import { LongFormChunking } from "./LongFormChunking";
import { VadBackendSelector } from "./VadBackendSelector";
import { RecordingBuffer } from "./debug/RecordingBuffer";
import { FillerWordRemoval } from "./FillerWordRemoval";
import { AccelerationSelector } from "./AccelerationSelector";
import { ModelUnloadTimeoutSetting } from "./ModelUnloadTimeout";
import { PasteMethodSetting } from "./PasteMethod";
import { ReliablePasteToggle } from "./debug/ReliablePaste";
import { ClipboardHandlingSetting } from "./ClipboardHandling";
import { FailedPastes } from "./FailedPastes";
import { AutoSubmit } from "./AutoSubmit";
import { PressEnterByVoice, StackedMessages } from "./SendByVoice";
import { AppendTrailingSpace } from "./AppendTrailingSpace";
import {
  AiPrivacyNote,
  AiProviders,
  AiStatusCard,
  CorrectionPassRow,
  MinWordsRow,
  TimeoutRow,
  NotesTimeoutRow,
  TransformsUseRow,
  NotesSummaryRow,
  TaskModelRow,
} from "./ai/AiSettings";
import {
  AutoStartRow,
  BrowserTitlesRow,
  CountdownRow,
  DetectCallsRow,
  HideCardRow,
  LastCallRow,
  MaxLengthRow,
  MeetingCardsRow,
  OpenNoteRow,
  SnoozeRow,
  StopOnEndRow,
  TestDetectionRow,
  WatchedAppsRow,
} from "./calls/CallSettings";
import {
  KeepAudioRow,
  RememberVoicesRow,
  SavedVoicesRow,
  SpeakerModelRow,
  SpeakersToggleRow,
} from "./calls/SpeakerSettings";
import {
  ClipboardImagesRow,
  NeverShareAppsRow,
  NeverSyncsRow,
  PauseClipboardRow,
  PcNameRow,
  PhonePillRow,
  PhoneCard,
  PrivateContentRow,
  RecentClipsRow,
  SendClipboardRow,
  SharedClipboardRow,
  ShareAiRow,
  SYNC_KINDS,
  SyncKindRow,
  SyncStatusRow,
} from "./devices/DeviceSettings";
import {
  CalendarsCard,
  ComingUpRow,
  hasCalendars,
  McpSetupRow,
  McpToggle,
} from "./connections/ConnectionSettings";
import { ShowOverlay } from "./ShowOverlay";
import {
  ConfirmPaste,
  LiveCaptionPlace,
  ShowBubble,
  ShowChanges,
} from "./BarMessages";
import { RememberApp } from "./RememberApp";
import { HistoryLimit } from "./HistoryLimit";
import { RecordingRetentionPeriodSelector } from "./RecordingRetentionPeriod";
import { UpdateChecksToggle } from "./UpdateChecksToggle";
import { ShowWhatsNewOnUpdate } from "./ShowWhatsNewOnUpdate";
import { AppDataDirectory } from "./AppDataDirectory";
import { LogDirectory } from "./debug/LogDirectory";
import { LogLevelSelector } from "./debug/LogLevelSelector";
import { LiveLogViewer } from "./debug/LiveLogViewer";
import { PasteDelay } from "./debug/PasteDelay";
import { AlwaysOnMicrophone } from "./AlwaysOnMicrophone";
import { WhatsNewPreview } from "./debug/WhatsNewPreview";
import {
  OnboardingPreview,
  type OnboardingPreviewStep,
} from "./debug/OnboardingPreview";
import { DebugModeRow } from "./debug/DebugModeRow";
import {
  AcknowledgmentsRow,
  SourceCodeRow,
  VersionRow,
} from "./about/AboutRows";

/** What a dependent row looks at to decide whether it can be changed. */
export interface RowContext {
  settings: AppSettings | null;
  model: ModelInfo | undefined;
  ai: AiStatus | null;
  /** A phone is paired (Settings › Devices). */
  paired: boolean;
}

/** A dependency that isn't met: the reason key, plus values for it. */
export interface Unmet {
  reasonKey: string;
  values?: Record<string, string>;
}

export interface SettingRow {
  id: string;
  /**
   * Translation keys the Settings search looks through: the row's title and
   * help, plus option labels people might type ("Dark" finds Theme).
   */
  searchKeys: string[];
  render: () => React.ReactNode;
  /** Returns why the row can't be changed right now, or null when it can. */
  requires?: (ctx: RowContext) => Unmet | null;
  /** Shows the row only when this holds. */
  when?: (ctx: RowContext) => boolean;
}

export interface SettingsGroupDef {
  id: string;
  titleKey: string;
  descriptionKey?: string;
  rows: SettingRow[];
  /** Shows the group only when this holds. */
  when?: (ctx: RowContext) => boolean;
}

export interface SectionDef {
  id: SettingsSection;
  icon: LucideIcon;
  groups: SettingsGroupDef[];
  /** Debug only shows in debug mode (Ctrl+Shift+D). */
  hidden?: (settings: AppSettings | null) => boolean;
}

const CLIPBOARD_PASTE = ["ctrl_v", "ctrl_shift_v", "shift_insert"];

const needsTransforms = ({ settings }: RowContext): Unmet | null =>
  settings?.post_process_enabled
    ? null
    : { reasonKey: "settings.requires.transforms" };

const needsAsk = (ctx: RowContext): Unmet | null =>
  needsTransforms(ctx) ??
  ((ctx.settings?.ask_enabled ?? true)
    ? null
    : { reasonKey: "settings.requires.ask" });

const needsCallDetection = ({ settings }: RowContext): Unmet | null =>
  (settings?.calls?.detect ?? true)
    ? null
    : { reasonKey: "settings.requires.calls" };

const paired = ({ paired }: RowContext) => paired;

/** The bar has the island's look, which stands in for the bubble. */
const islandOn = (settings: AppSettings | null) =>
  settings?.bar_look === "island" && settings.overlay_style !== "none";

const needsClipboardSync = ({ settings }: RowContext): Unmet | null =>
  (settings?.sync?.clipboard ?? true)
    ? null
    : { reasonKey: "settings.requires.clipboardSync" };

const needsSoundFeedback = ({ settings }: RowContext): Unmet | null =>
  settings?.audio_feedback ? null : { reasonKey: "settings.requires.sound" };

const needsAiModel = ({ ai }: RowContext): Unmet | null =>
  !ai || ai.kind === "none"
    ? { reasonKey: "settings.ai.use.needsModel" }
    : null;

const needsVad = ({ settings }: RowContext): Unmet | null =>
  (settings?.vad_enabled ?? true)
    ? null
    : { reasonKey: "settings.requires.vad" };

/** The language row needs the active model's language list. */
const SpokenLanguage: React.FC = () => {
  const { currentModel, models } = useModelStore();
  const model = models.find((m) => m.id === currentModel);
  return (
    <LanguageSelector
      grouped
      supportedLanguages={model?.supported_languages}
      supportsLanguageDetection={model?.supports_language_detection ?? true}
    />
  );
};

export const buildSections = (
  onPreviewOnboarding: (step: OnboardingPreviewStep) => void,
): SectionDef[] => [
  {
    id: "general",
    icon: Settings2,
    groups: [
      {
        id: "you",
        titleKey: "settings.groups.you",
        rows: [
          {
            id: "your-name",
            searchKeys: [
              "settings.yourName.title",
              "settings.yourName.description",
            ],
            render: () => <YourName grouped />,
          },
        ],
      },
      {
        id: "startup",
        titleKey: "settings.groups.startup",
        rows: [
          {
            id: "autostart",
            searchKeys: [
              "settings.advanced.autostart.label",
              "settings.advanced.autostart.description",
            ],
            render: () => <AutostartToggle grouped />,
          },
          {
            id: "tray",
            searchKeys: [
              "settings.advanced.showTrayIcon.label",
              "settings.advanced.showTrayIcon.description",
            ],
            render: () => <ShowTrayIcon grouped />,
          },
          {
            id: "start-hidden",
            searchKeys: [
              "settings.advanced.startHidden.label",
              "settings.advanced.startHidden.description",
            ],
            render: () => <StartHidden grouped />,
            requires: ({ settings }) =>
              (settings?.show_tray_icon ?? true)
                ? null
                : { reasonKey: "settings.requires.tray" },
          },
          {
            id: "admin-apps",
            searchKeys: [
              "settings.advanced.adminApps.label",
              "settings.advanced.adminApps.description",
            ],
            render: () => <AdminAppsToggle grouped />,
          },
        ],
      },
      {
        id: "appearance",
        titleKey: "settings.groups.appearance",
        rows: [
          {
            id: "theme",
            searchKeys: [
              "theme.title",
              "theme.description",
              "theme.options.light",
              "theme.options.dark",
            ],
            render: () => <ThemeSelector grouped />,
          },
        ],
      },
    ],
  },
  {
    id: "features",
    icon: Blocks,
    groups: [
      {
        id: "features",
        titleKey: "settings.groups.features",
        rows: [
          {
            id: "feature-notes",
            searchKeys: [
              "settings.features.notes.title",
              "settings.features.notes.help",
            ],
            render: () => (
              <FeatureToggle feature="notes_enabled" name="notes" />
            ),
          },
          {
            id: "feature-scratchpad",
            searchKeys: [
              "settings.features.scratchpad.title",
              "settings.features.scratchpad.help",
            ],
            render: () => (
              <FeatureToggle feature="scratchpad_enabled" name="scratchpad" />
            ),
          },
          {
            id: "feature-transforms",
            searchKeys: [
              "settings.features.transforms.title",
              "settings.features.transforms.help",
            ],
            render: () => (
              <FeatureToggle feature="post_process_enabled" name="transforms" />
            ),
          },
          {
            id: "feature-insights",
            searchKeys: [
              "settings.features.insights.title",
              "settings.features.insights.help",
            ],
            render: () => (
              <FeatureToggle feature="insights_enabled" name="insights" />
            ),
          },
        ],
      },
      {
        id: "features-other",
        titleKey: "settings.groups.featuresOther",
        rows: [
          {
            id: "feature-mcp",
            searchKeys: [
              "settings.connections.mcp.title",
              "settings.connections.mcp.help",
            ],
            render: () => <McpToggle />,
          },
        ],
      },
    ],
  },
  {
    id: "shortcuts",
    icon: Keyboard,
    groups: [
      {
        id: "dictation",
        titleKey: "settings.groups.dictation",
        rows: [
          {
            id: "dictate",
            searchKeys: [
              "settings.general.shortcut.bindings.transcribe.name",
              "settings.general.shortcut.bindings.transcribe.description",
            ],
            render: () => <ShortcutInput shortcutId="transcribe" grouped />,
          },
          {
            id: "activation",
            searchKeys: [
              "settings.general.shortcutActivation.title",
              "settings.general.shortcutActivation.description",
              "settings.general.shortcutActivation.options.pushToTalk",
              "settings.general.shortcutActivation.options.toggle",
            ],
            render: () => <ShortcutActivationSetting grouped />,
          },
          {
            id: "hold-threshold",
            searchKeys: [
              "settings.debug.holdThreshold.title",
              "settings.debug.holdThreshold.description",
            ],
            render: () => <HoldThreshold grouped />,
            requires: ({ settings }) =>
              (settings?.shortcut_activation ?? "hold_or_toggle") ===
              "hold_or_toggle"
                ? null
                : { reasonKey: "settings.requires.auto" },
          },
          {
            id: "dictate-transform",
            searchKeys: [
              "settings.general.shortcut.bindings.transcribe_with_post_process.name",
              "settings.general.shortcut.bindings.transcribe_with_post_process.description",
            ],
            render: () => (
              <ShortcutInput
                shortcutId="transcribe_with_post_process"
                grouped
              />
            ),
            requires: needsTransforms,
          },
          {
            id: "cancel",
            searchKeys: [
              "settings.general.shortcut.bindings.cancel.name",
              "settings.general.shortcut.bindings.cancel.description",
            ],
            render: () => <ShortcutInput shortcutId="cancel" grouped />,
          },
          {
            id: "add-to-dictionary",
            searchKeys: [
              "settings.general.shortcut.bindings.add_to_dictionary.name",
              "settings.general.shortcut.bindings.add_to_dictionary.description",
            ],
            render: () => (
              <ShortcutInput shortcutId="add_to_dictionary" grouped />
            ),
          },
        ],
      },
      {
        id: "transforms",
        titleKey: "settings.groups.transforms",
        rows: [
          {
            id: "transform-selection",
            searchKeys: [
              "settings.general.shortcut.bindings.transform_selection.name",
              "settings.general.shortcut.bindings.transform_selection.description",
            ],
            render: () => (
              <ShortcutInput shortcutId="transform_selection" grouped />
            ),
            requires: needsTransforms,
          },
          {
            id: "ask",
            searchKeys: [
              "settings.general.shortcut.bindings.ask.name",
              "settings.general.shortcut.bindings.ask.description",
            ],
            render: () => <ShortcutInput shortcutId="ask" grouped />,
            requires: needsAsk,
          },
          {
            id: "view-changes",
            searchKeys: [
              "settings.general.shortcut.bindings.view_changes.name",
              "settings.general.shortcut.bindings.view_changes.description",
            ],
            render: () => <ShortcutInput shortcutId="view_changes" grouped />,
            requires: needsTransforms,
          },
        ],
      },
      {
        id: "scratchpad",
        titleKey: "settings.groups.scratchpad",
        rows: [
          {
            id: "open-scratchpad",
            searchKeys: [
              "settings.general.shortcut.bindings.open_scratchpad.name",
              "settings.general.shortcut.bindings.open_scratchpad.description",
            ],
            render: () => (
              <ShortcutInput shortcutId="open_scratchpad" grouped />
            ),
            requires: ({ settings }) =>
              (settings?.scratchpad_enabled ?? true)
                ? null
                : { reasonKey: "settings.requires.scratchpadFeature" },
          },
        ],
      },
      {
        id: "notes",
        titleKey: "settings.groups.notes",
        rows: [
          {
            id: "toggle-notes",
            searchKeys: [
              "settings.general.shortcut.bindings.toggle_notes.name",
              "settings.general.shortcut.bindings.toggle_notes.description",
            ],
            render: () => <ShortcutInput shortcutId="toggle_notes" grouped />,
            requires: ({ settings }) =>
              (settings?.notes_enabled ?? true)
                ? null
                : { reasonKey: "settings.requires.notesFeature" },
          },
        ],
      },
      {
        id: "companion",
        titleKey: "settings.groups.companion",
        rows: [
          {
            id: "talk-to-companion",
            searchKeys: [
              "settings.general.shortcut.bindings.talk_to_companion.name",
              "settings.general.shortcut.bindings.talk_to_companion.description",
            ],
            render: () => (
              <ShortcutInput shortcutId="talk_to_companion" grouped />
            ),
            requires: ({ settings }) =>
              settings?.inyon?.hub
                ? null
                : { reasonKey: "settings.requires.inyonPaired" },
          },
        ],
      },
      {
        id: "last-dictation",
        titleKey: "settings.groups.lastDictation",
        rows: [
          {
            id: "paste-last",
            searchKeys: [
              "settings.general.shortcut.bindings.paste_last.name",
              "settings.general.shortcut.bindings.paste_last.description",
            ],
            render: () => <ShortcutInput shortcutId="paste_last" grouped />,
          },
          {
            id: "copy-last",
            searchKeys: [
              "settings.general.shortcut.bindings.copy_last.name",
              "settings.general.shortcut.bindings.copy_last.description",
            ],
            render: () => <ShortcutInput shortcutId="copy_last" grouped />,
          },
        ],
      },
      {
        id: "keyboard",
        titleKey: "settings.groups.keyboard",
        rows: [
          {
            id: "keyboard-backend",
            searchKeys: [
              "settings.debug.keyboardImplementation.title",
              "settings.debug.keyboardImplementation.description",
            ],
            render: () => <KeyboardImplementationSelector grouped />,
          },
        ],
      },
    ],
  },
  {
    id: "audio",
    icon: Mic,
    groups: [
      {
        id: "microphone",
        titleKey: "settings.groups.microphone",
        rows: [
          {
            id: "microphone",
            searchKeys: [
              "settings.sound.microphone.title",
              "settings.sound.microphone.description",
            ],
            render: () => <MicrophoneSelector grouped />,
          },
          {
            id: "microphone-order",
            searchKeys: [
              "settings.sound.microphoneOrder.title",
              "settings.sound.microphoneOrder.description",
            ],
            render: () => <MicrophoneOrder grouped />,
          },
          {
            id: "channel",
            searchKeys: [
              "settings.sound.channel.title",
              "settings.sound.channel.description",
            ],
            render: () => <ChannelSelector grouped />,
          },
        ],
      },
      {
        id: "while-dictating",
        titleKey: "settings.groups.whileDictating",
        rows: [
          {
            id: "background-audio",
            searchKeys: [
              "settings.sound.backgroundAudio.title",
              "settings.sound.backgroundAudio.search",
            ],
            render: () => <BackgroundAudio grouped />,
          },
          {
            id: "keep-mic-open",
            searchKeys: [
              "settings.advanced.lazyStreamClose.label",
              "settings.advanced.lazyStreamClose.description",
            ],
            render: () => <LazyStreamClose grouped />,
          },
        ],
      },
      {
        id: "sounds",
        titleKey: "settings.groups.sounds",
        rows: [
          {
            id: "sound-feedback",
            searchKeys: [
              "settings.sound.audioFeedback.label",
              "settings.sound.audioFeedback.description",
            ],
            render: () => <AudioFeedback grouped />,
          },
          {
            id: "sound",
            searchKeys: [
              "settings.debug.soundTheme.label",
              "settings.debug.soundTheme.description",
            ],
            render: () => <SoundPicker />,
            requires: needsSoundFeedback,
          },
          {
            id: "volume",
            searchKeys: [
              "settings.sound.volume.title",
              "settings.sound.volume.description",
            ],
            render: () => <VolumeSlider />,
            requires: needsSoundFeedback,
          },
          {
            id: "output-device",
            searchKeys: [
              "settings.sound.outputDevice.title",
              "settings.sound.outputDevice.description",
            ],
            render: () => <OutputDeviceSelector grouped />,
            requires: needsSoundFeedback,
          },
        ],
      },
    ],
  },
  {
    id: "transcription",
    icon: Captions,
    groups: [
      {
        id: "language",
        titleKey: "settings.groups.language",
        rows: [
          {
            id: "language",
            searchKeys: [
              "settings.general.language.title",
              "settings.general.language.description",
            ],
            render: () => <SpokenLanguage />,
            requires: ({ model }) =>
              !model
                ? { reasonKey: "settings.requires.model" }
                : model.supports_language_selection
                  ? null
                  : {
                      reasonKey: "settings.requires.languageSelection",
                      values: { model: model.name },
                    },
          },
          {
            id: "translate",
            searchKeys: [
              "settings.advanced.translateToEnglish.label",
              "settings.advanced.translateToEnglish.description",
            ],
            render: () => <TranslateToEnglish grouped />,
            requires: ({ model }) =>
              !model
                ? { reasonKey: "settings.requires.model" }
                : model.supports_translation
                  ? null
                  : {
                      reasonKey: "settings.requires.translation",
                      values: { model: model.name },
                    },
          },
          {
            id: "chinese-script",
            searchKeys: [
              "settings.advanced.chineseScript.title",
              "settings.advanced.chineseScript.description",
            ],
            render: () => <ChineseScriptSetting grouped />,
            requires: ({ settings }) => {
              const lang = settings?.selected_language ?? "auto";
              return lang === "auto" || lang.startsWith("zh") || lang === "yue"
                ? null
                : { reasonKey: "settings.requires.chinese" };
            },
          },
        ],
      },
      {
        id: "recording",
        titleKey: "settings.groups.recording",
        rows: [
          {
            id: "vad",
            searchKeys: [
              "settings.advanced.voiceActivityDetection.title",
              "settings.advanced.voiceActivityDetection.description",
            ],
            render: () => <VoiceActivityDetection grouped />,
          },
          {
            id: "long-form",
            searchKeys: [
              "settings.advanced.longFormChunking.title",
              "settings.advanced.longFormChunking.description",
            ],
            render: () => <LongFormChunking grouped />,
            requires: needsVad,
          },
          {
            id: "vad-backend",
            searchKeys: [
              "settings.advanced.vadBackend.title",
              "settings.advanced.vadBackend.description",
            ],
            render: () => <VadBackendSelector grouped />,
            requires: needsVad,
          },
          {
            id: "recording-buffer",
            searchKeys: [
              "settings.debug.recordingBuffer.title",
              "settings.debug.recordingBuffer.description",
            ],
            render: () => <RecordingBuffer grouped />,
          },
        ],
      },
      {
        id: "cleanup",
        titleKey: "settings.groups.cleanup",
        rows: [
          {
            id: "fillers",
            searchKeys: [
              "settings.advanced.fillerWordRemoval.title",
              "settings.advanced.fillerWordRemoval.description",
            ],
            render: () => <FillerWordRemoval grouped />,
          },
        ],
      },
      {
        id: "engine",
        titleKey: "settings.groups.engine",
        rows: [
          {
            id: "acceleration",
            searchKeys: [
              "settings.advanced.acceleration.transcribe.title",
              "settings.advanced.acceleration.transcribe.description",
            ],
            render: () => <AccelerationSelector grouped />,
          },
          {
            id: "unload",
            searchKeys: [
              "settings.advanced.modelUnload.title",
              "settings.advanced.modelUnload.description",
            ],
            render: () => <ModelUnloadTimeoutSetting grouped />,
          },
        ],
      },
    ],
  },
  {
    id: "output",
    icon: ClipboardPaste,
    groups: [
      {
        id: "paste",
        titleKey: "settings.groups.paste",
        rows: [
          {
            id: "paste-method",
            searchKeys: [
              "settings.advanced.pasteMethod.title",
              "settings.advanced.pasteMethod.description",
            ],
            render: () => <PasteMethodSetting grouped />,
          },
          {
            id: "reliable-paste",
            searchKeys: [
              "settings.debug.reliablePaste.title",
              "settings.debug.reliablePaste.description",
            ],
            render: () => <ReliablePasteToggle grouped />,
            requires: ({ settings }) =>
              CLIPBOARD_PASTE.includes(settings?.paste_method ?? "ctrl_v")
                ? null
                : { reasonKey: "settings.requires.clipboardPaste" },
          },
          {
            id: "clipboard",
            searchKeys: [
              "settings.advanced.clipboardHandling.title",
              "settings.advanced.clipboardHandling.description",
            ],
            render: () => <ClipboardHandlingSetting grouped />,
          },
          {
            id: "failed-pastes",
            searchKeys: [
              "settings.output.failedPastes.title",
              "settings.output.failedPastes.description",
            ],
            render: () => <FailedPastes grouped />,
          },
        ],
      },
      {
        id: "after-pasting",
        titleKey: "settings.groups.afterPasting",
        rows: [
          {
            id: "auto-submit",
            searchKeys: [
              "settings.advanced.autoSubmit.title",
              "settings.advanced.autoSubmit.description",
            ],
            render: () => <AutoSubmit grouped />,
          },
          {
            id: "press-enter-by-voice",
            searchKeys: [
              "settings.advanced.pressEnterByVoice.label",
              "settings.advanced.pressEnterByVoice.description",
            ],
            render: () => <PressEnterByVoice grouped />,
          },
          {
            id: "stacked-messages",
            searchKeys: [
              "settings.advanced.stackedMessages.label",
              "settings.advanced.stackedMessages.description",
            ],
            render: () => <StackedMessages grouped />,
          },
          {
            id: "trailing-space",
            searchKeys: [
              "settings.debug.appendTrailingSpace.label",
              "settings.debug.appendTrailingSpace.description",
            ],
            render: () => <AppendTrailingSpace grouped />,
          },
        ],
      },
    ],
  },
  {
    id: "ai",
    icon: Plug,
    groups: [
      {
        id: "connection",
        titleKey: "settings.groups.untitled",
        rows: [
          {
            id: "ai-status",
            searchKeys: [
              "settings.ai.actions.test",
              "settings.ai.status.connectedTitle",
            ],
            render: () => <AiStatusCard />,
          },
          {
            id: "provider",
            searchKeys: [
              "settings.ai.provider.title",
              "settings.ai.provider.help",
              "settings.ai.key.title",
              "settings.ai.model.title",
              "settings.ai.baseUrl.title",
            ],
            render: () => <AiProviders />,
          },
        ],
      },
      {
        id: "ai-use",
        titleKey: "settings.ai.use.title",
        rows: [
          {
            id: "ai-correction",
            searchKeys: [
              "settings.ai.use.correction.title",
              "settings.ai.use.correction.helpEmpty",
            ],
            render: () => <CorrectionPassRow />,
            requires: needsAiModel,
          },
          {
            id: "ai-transforms",
            searchKeys: [
              "settings.ai.use.transforms.title",
              "settings.ai.use.transforms.help",
            ],
            render: () => <TransformsUseRow />,
            requires: needsAiModel,
          },
          {
            id: "ai-notes",
            searchKeys: [
              "settings.ai.use.notes.title",
              "settings.ai.use.notes.help",
            ],
            render: () => <NotesSummaryRow />,
            requires: needsAiModel,
          },
        ],
      },
      {
        id: "ai-tasks",
        titleKey: "settings.ai.tasks.title",
        rows: (["dictation", "transforms", "notes"] as const).map((task) => ({
          id: `ai-task-${task}`,
          searchKeys: [
            `settings.ai.tasks.${task}.title`,
            `settings.ai.tasks.${task}.help`,
          ],
          render: () => <TaskModelRow task={task} />,
          requires: needsAiModel,
        })),
      },
      {
        id: "ai-limits",
        titleKey: "settings.ai.limits.title",
        rows: [
          {
            id: "ai-min-words",
            searchKeys: [
              "settings.ai.limits.minWords.title",
              "settings.ai.limits.minWords.helpOff",
            ],
            render: () => <MinWordsRow />,
            requires: needsAiModel,
          },
          {
            id: "ai-timeout",
            searchKeys: [
              "settings.ai.limits.timeout.title",
              "settings.ai.limits.timeout.help",
            ],
            render: () => <TimeoutRow />,
            requires: needsAiModel,
          },
          {
            id: "ai-notes-timeout",
            searchKeys: [
              "settings.ai.limits.notesTimeout.title",
              "settings.ai.limits.notesTimeout.help",
            ],
            render: () => <NotesTimeoutRow />,
            requires: needsAiModel,
          },
          {
            id: "ai-privacy",
            searchKeys: [],
            render: () => <AiPrivacyNote />,
          },
        ],
      },
    ],
  },
  {
    id: "calls",
    icon: PhoneIncoming,
    groups: [
      {
        id: "calls-master",
        titleKey: "settings.groups.untitled",
        rows: [
          {
            id: "detect-calls",
            searchKeys: [
              "settings.calls.detect.title",
              "settings.calls.detect.help",
            ],
            render: () => <DetectCallsRow />,
          },
        ],
      },
      {
        id: "calls-detection",
        titleKey: "settings.groups.detection",
        rows: [
          {
            id: "browser-titles",
            searchKeys: [
              "settings.calls.browserTitles.title",
              "settings.calls.browserTitles.help",
            ],
            render: () => <BrowserTitlesRow />,
            requires: needsCallDetection,
          },
          {
            id: "watched-apps",
            searchKeys: [
              "settings.calls.apps.title",
              "calls.app.teams",
              "calls.app.zoom",
              "calls.app.meet",
              "calls.app.slack",
              "calls.app.discord",
              "calls.app.whatsapp",
              "calls.app.telegram",
            ],
            render: () => <WatchedAppsRow />,
            requires: needsCallDetection,
          },
          {
            id: "last-call",
            searchKeys: ["settings.calls.last.title"],
            render: () => <LastCallRow />,
          },
          {
            id: "test-detection",
            searchKeys: [
              "settings.calls.test.title",
              "settings.calls.test.button",
            ],
            render: () => <TestDetectionRow />,
            requires: needsCallDetection,
          },
        ],
      },
      {
        id: "call-start",
        titleKey: "settings.groups.callStart",
        rows: [
          {
            id: "auto-start",
            searchKeys: [
              "settings.calls.autoStart.title",
              "settings.calls.autoStart.help",
            ],
            render: () => <AutoStartRow />,
            requires: needsCallDetection,
          },
          {
            id: "countdown",
            searchKeys: ["settings.calls.countdown.title"],
            render: () => <CountdownRow />,
            requires: (ctx) =>
              needsCallDetection(ctx) ??
              ((ctx.settings?.calls?.auto_start ?? false)
                ? null
                : { reasonKey: "settings.requires.autoStart" }),
          },
          {
            id: "snooze",
            searchKeys: [
              "settings.calls.snooze.title",
              "settings.calls.snooze.help",
            ],
            render: () => <SnoozeRow />,
            requires: needsCallDetection,
          },
          {
            id: "hide-card",
            searchKeys: [
              "settings.calls.hideCard.title",
              "settings.calls.hideCard.help",
            ],
            render: () => <HideCardRow />,
            requires: needsCallDetection,
          },
        ],
      },
      {
        id: "call-end",
        titleKey: "settings.groups.callEnd",
        rows: [
          {
            id: "stop-on-end",
            searchKeys: [
              "settings.calls.stopOnEnd.title",
              "settings.calls.stopOnEnd.help",
            ],
            render: () => <StopOnEndRow />,
            requires: needsCallDetection,
          },
          {
            id: "max-length",
            searchKeys: [
              "settings.calls.maxLength.title",
              "settings.calls.maxLength.help",
            ],
            render: () => <MaxLengthRow />,
          },
          {
            id: "open-note",
            searchKeys: [
              "settings.calls.openNote.title",
              "settings.calls.openNote.help",
            ],
            render: () => <OpenNoteRow />,
            requires: needsCallDetection,
          },
        ],
      },
      {
        id: "speakers",
        titleKey: "settings.groups.speakers",
        rows: [
          {
            id: "speaker-model",
            searchKeys: [
              "settings.speakers.model.title",
              "settings.speakers.toggle.title",
            ],
            render: () => <SpeakerModelRow />,
          },
          {
            id: "tell-speakers",
            searchKeys: [
              "settings.speakers.toggle.title",
              "settings.speakers.toggle.help",
            ],
            render: () => <SpeakersToggleRow />,
          },
          {
            id: "remember-voices",
            searchKeys: [
              "settings.speakers.remember.title",
              "settings.speakers.remember.help",
            ],
            render: () => <RememberVoicesRow />,
          },
          {
            id: "saved-voices",
            searchKeys: [
              "settings.speakers.voices.title",
              "settings.speakers.voices.help",
            ],
            render: () => <SavedVoicesRow />,
          },
        ],
      },
      {
        id: "recordings",
        titleKey: "settings.groups.recordings",
        rows: [
          {
            id: "keep-audio",
            searchKeys: [
              "settings.calls.keepAudio.title",
              "settings.calls.keepAudio.help",
            ],
            render: () => <KeepAudioRow />,
          },
        ],
      },
    ],
  },
  {
    id: "connections",
    icon: CalendarDays,
    groups: [
      {
        id: "calendars",
        titleKey: "settings.groups.calendars",
        rows: [
          {
            id: "calendars",
            searchKeys: [
              "settings.connections.calendars.title",
              "settings.connections.calendars.help",
              "settings.connections.calendars.add",
            ],
            render: () => <CalendarsCard />,
          },
          {
            id: "coming-up",
            searchKeys: [
              "settings.connections.comingUp.title",
              "settings.connections.comingUp.help",
            ],
            render: () => <ComingUpRow />,
            when: ({ settings }) => hasCalendars(settings?.calendars),
          },
          {
            id: "meeting-cards",
            searchKeys: [
              "settings.calls.meetingCards.title",
              "settings.calls.meetingCards.help",
            ],
            render: () => <MeetingCardsRow />,
            when: ({ settings }) => hasCalendars(settings?.calendars),
          },
        ],
      },
      {
        id: "documents",
        titleKey: "settings.groups.documents",
        rows: [
          {
            id: "notes-folder",
            searchKeys: [
              "settings.connections.notesFolder.title",
              "settings.connections.notesFolder.help",
            ],
            render: () => <NotesFolderRow />,
          },
        ],
      },
      {
        id: "inyon",
        titleKey: "settings.groups.inyon",
        rows: [
          {
            id: "inyon",
            searchKeys: [
              "settings.connections.inyon.title",
              "settings.connections.inyon.help",
              "settings.connections.inyon.pair",
            ],
            render: () => <InyonCard />,
          },
          {
            id: "inyon-alerts",
            searchKeys: [
              "settings.connections.inyon.alerts.title",
              "settings.connections.inyon.alerts.help",
            ],
            render: () => <InyonAlertsRow />,
            when: ({ settings }) => !!settings?.inyon?.hub,
          },
        ],
      },
      {
        id: "ai-apps",
        titleKey: "settings.groups.aiApps",
        rows: [
          {
            id: "mcp",
            searchKeys: [
              "settings.connections.mcp.title",
              "settings.connections.mcp.help",
            ],
            render: () => <McpToggle />,
          },
          {
            id: "mcp-setup",
            searchKeys: [
              "settings.connections.mcp.setup.title",
              "settings.connections.mcp.setup.help",
            ],
            render: () => <McpSetupRow />,
            requires: ({ settings }) =>
              settings?.mcp_enabled
                ? null
                : { reasonKey: "settings.requires.mcp" },
          },
        ],
      },
    ],
  },
  {
    id: "bar",
    icon: PanelBottom,
    groups: [
      {
        id: "look",
        titleKey: "settings.groups.look",
        rows: [
          {
            id: "bar-style",
            searchKeys: [
              "settings.advanced.overlay.style.title",
              "settings.advanced.overlay.style.description",
              "settings.advanced.overlay.style.options.minimal",
              "settings.advanced.overlay.style.options.live",
              "settings.advanced.overlay.position.title",
              "settings.advanced.overlay.position.description",
              "settings.advanced.overlay.look.title",
              "settings.advanced.overlay.look.description",
              "settings.advanced.overlay.idle.title",
              "settings.advanced.overlay.idle.description",
            ],
            render: () => <ShowOverlay grouped />,
          },
          {
            id: "bubble",
            searchKeys: [
              "settings.bar.bubble.title",
              "settings.bar.bubble.help",
            ],
            render: () => <ShowBubble grouped />,
            requires: ({ settings }) =>
              islandOn(settings)
                ? { reasonKey: "settings.requires.islandOff" }
                : null,
          },
          {
            id: "live-caption",
            searchKeys: [
              "settings.bar.liveCaption.title",
              "settings.bar.liveCaption.help",
            ],
            render: () => <LiveCaptionPlace grouped />,
            // A choice about the bubble's dictations; the island shows its
            // own words, so the row goes while it's on.
            when: ({ settings }) => !islandOn(settings),
            requires: ({ settings }) =>
              settings?.bubble_enabled === false
                ? { reasonKey: "settings.requires.bubble" }
                : null,
          },
        ],
      },
      {
        id: "messages",
        titleKey: "settings.groups.messages",
        rows: [
          {
            id: "confirm-paste",
            searchKeys: [
              "settings.bar.confirmPaste.title",
              "settings.bar.confirmPaste.help",
            ],
            render: () => <ConfirmPaste grouped />,
          },
          {
            id: "show-changes",
            searchKeys: [
              "settings.bar.showChanges.title",
              "settings.bar.showChanges.help",
            ],
            render: () => <ShowChanges grouped />,
          },
        ],
      },
    ],
  },
  {
    id: "devices",
    icon: Smartphone,
    groups: [
      {
        id: "phone",
        titleKey: "settings.groups.phone",
        rows: [
          {
            id: "phone",
            searchKeys: [
              "settings.devices.pairCard.title",
              "settings.devices.pairCard.help",
              "settings.devices.phone.syncNow",
              "settings.devices.phone.rename",
              "settings.devices.phone.unpair",
            ],
            render: () => <PhoneCard />,
          },
          {
            id: "sync-status",
            searchKeys: [
              "settings.devices.status.title",
              "settings.devices.status.seeMerges",
            ],
            render: () => <SyncStatusRow />,
            when: paired,
          },
          {
            id: "pc-name",
            searchKeys: [
              "settings.devices.pcName.title",
              "settings.devices.pcName.help",
            ],
            render: () => <PcNameRow />,
          },
        ],
      },
      {
        id: "what-syncs",
        titleKey: "settings.groups.whatSyncs",
        descriptionKey: "settings.devices.whatSyncs",
        when: paired,
        rows: [
          ...SYNC_KINDS.map((kind) => ({
            id: `sync-${kind}`,
            searchKeys: [`settings.devices.kinds.${kind}.title`],
            render: () => <SyncKindRow kind={kind} />,
          })),
          {
            id: "never-syncs",
            searchKeys: ["settings.devices.never"],
            render: () => <NeverSyncsRow />,
          },
          {
            id: "share-ai",
            searchKeys: [
              "settings.devices.shareAi.title",
              "settings.devices.shareAi.help",
            ],
            render: () => <ShareAiRow />,
          },
        ],
      },
      {
        id: "clipboard",
        titleKey: "settings.groups.clipboard",
        when: paired,
        rows: [
          {
            id: "shared-clipboard",
            searchKeys: [
              "settings.devices.clipboard.title",
              "settings.devices.clipboard.help",
            ],
            render: () => <SharedClipboardRow />,
          },
          {
            id: "clipboard-images",
            searchKeys: [
              "settings.devices.images.title",
              "settings.devices.images.help",
            ],
            render: () => <ClipboardImagesRow />,
            requires: needsClipboardSync,
          },
          {
            id: "private-content",
            searchKeys: [
              "settings.devices.private.title",
              "settings.devices.private.help",
            ],
            render: () => <PrivateContentRow />,
          },
          {
            id: "never-share",
            searchKeys: [
              "settings.devices.neverApps.title",
              "settings.devices.neverApps.help",
            ],
            render: () => <NeverShareAppsRow />,
            requires: needsClipboardSync,
          },
          {
            id: "phone-pill",
            searchKeys: [
              "settings.devices.pill.title",
              "settings.devices.pill.help",
            ],
            render: () => <PhonePillRow />,
            requires: needsClipboardSync,
          },
          {
            id: "pause-clipboard",
            searchKeys: [
              "settings.devices.pause.title",
              "settings.devices.pause.help",
            ],
            render: () => <PauseClipboardRow />,
            requires: needsClipboardSync,
          },
          {
            id: "send-clipboard",
            searchKeys: [
              "settings.devices.send.title",
              "settings.devices.send.help",
            ],
            render: () => <SendClipboardRow />,
            requires: needsClipboardSync,
          },
          {
            id: "recent-clips",
            searchKeys: [
              "settings.devices.clips.title",
              "settings.devices.clips.help",
            ],
            render: () => <RecentClipsRow />,
          },
        ],
      },
    ],
  },
  {
    id: "privacy",
    icon: Shield,
    groups: [
      {
        id: "history",
        titleKey: "settings.groups.history",
        rows: [
          {
            id: "history-limit",
            searchKeys: [
              "settings.debug.historyLimit.title",
              "settings.debug.historyLimit.description",
            ],
            render: () => <HistoryLimit grouped />,
          },
          {
            id: "retention",
            searchKeys: [
              "settings.debug.recordingRetention.title",
              "settings.debug.recordingRetention.description",
            ],
            render: () => <RecordingRetentionPeriodSelector grouped />,
          },
        ],
      },
      {
        id: "apps",
        titleKey: "settings.groups.apps",
        rows: [
          {
            id: "remember-app",
            searchKeys: [
              "settings.privacy.rememberApp.title",
              "settings.privacy.rememberApp.description",
            ],
            render: () => <RememberApp />,
          },
        ],
      },
    ],
  },
  {
    id: "about",
    icon: Info,
    groups: [
      {
        id: "updates",
        titleKey: "settings.groups.updates",
        rows: [
          {
            id: "version",
            searchKeys: [
              "settings.about.version.search",
              "footer.checkForUpdates",
            ],
            render: () => <VersionRow />,
          },
          {
            id: "update-checks",
            searchKeys: [
              "settings.debug.updateChecks.label",
              "settings.debug.updateChecks.description",
            ],
            render: () => <UpdateChecksToggle grouped />,
          },
          {
            id: "whats-new",
            searchKeys: [
              "settings.about.whatsNewUpdates.label",
              "settings.about.whatsNewUpdates.description",
            ],
            render: () => <ShowWhatsNewOnUpdate grouped />,
          },
        ],
      },
      {
        id: "troubleshooting",
        titleKey: "settings.groups.troubleshooting",
        rows: [
          {
            id: "app-data",
            searchKeys: [
              "settings.about.appDataDirectory.title",
              "settings.about.appDataDirectory.description",
            ],
            render: () => <AppDataDirectory grouped />,
          },
          {
            id: "logs",
            searchKeys: [
              "settings.debug.logDirectory.title",
              "settings.debug.logDirectory.description",
            ],
            render: () => <LogDirectory grouped />,
          },
        ],
      },
      {
        id: "open-source",
        titleKey: "settings.groups.openSource",
        rows: [
          {
            id: "source",
            searchKeys: [
              "settings.about.sourceCode.title",
              "settings.about.sourceCode.description",
            ],
            render: () => <SourceCodeRow />,
          },
          {
            id: "acknowledgments",
            searchKeys: [
              "settings.about.acknowledgments.ggml.title",
              "settings.about.acknowledgments.ggml.description",
            ],
            render: () => <AcknowledgmentsRow />,
          },
        ],
      },
    ],
  },
  {
    id: "debug",
    icon: Bug,
    hidden: (settings) => !(settings?.debug_mode ?? false),
    groups: [
      {
        id: "debug-mode",
        titleKey: "settings.groups.debugMode",
        rows: [
          {
            id: "debug-mode",
            searchKeys: [
              "settings.debug.mode.title",
              "settings.debug.mode.description",
            ],
            render: () => <DebugModeRow />,
          },
        ],
      },
      {
        id: "logs",
        titleKey: "settings.groups.logs",
        rows: [
          {
            id: "log-level",
            searchKeys: [
              "settings.debug.logLevel.title",
              "settings.debug.logLevel.description",
            ],
            render: () => <LogLevelSelector grouped />,
          },
          {
            id: "live-logs",
            searchKeys: [
              "settings.debug.liveLogs.title",
              "settings.debug.liveLogs.description",
            ],
            render: () => <LiveLogViewer grouped />,
          },
        ],
      },
      {
        id: "input",
        titleKey: "settings.groups.input",
        rows: [
          {
            id: "paste-delay",
            searchKeys: [
              "settings.debug.pasteDelay.title",
              "settings.debug.pasteDelay.description",
            ],
            render: () => <PasteDelay grouped />,
          },
          {
            id: "always-on-mic",
            searchKeys: [
              "settings.debug.alwaysOnMicrophone.label",
              "settings.debug.alwaysOnMicrophone.description",
            ],
            render: () => <AlwaysOnMicrophone grouped />,
          },
        ],
      },
      {
        id: "previews",
        titleKey: "settings.groups.previews",
        rows: [
          {
            id: "onboarding-preview",
            searchKeys: [
              "settings.debug.onboardingPreview.title",
              "settings.debug.onboardingPreview.description",
            ],
            render: () => (
              <OnboardingPreview onPreview={onPreviewOnboarding} grouped />
            ),
          },
          {
            id: "whats-new-preview",
            searchKeys: [
              "settings.debug.whatsNewPreview.title",
              "settings.debug.whatsNewPreview.description",
            ],
            render: () => <WhatsNewPreview grouped />,
          },
        ],
      },
    ],
  },
];
