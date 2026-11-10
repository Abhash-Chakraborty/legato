// Written by scripts/animated-icons.ts. Import icons from here, not from
// lucide-react: the ones listed below move when the control they sit in is
// hovered, and every other Lucide icon comes through as it is.

import {
  ArrowDown as ArrowDownStill,
  ArrowDownLeft as ArrowDownLeftStill,
  ArrowDownRight as ArrowDownRightStill,
  ArrowLeftRight as ArrowLeftRightStill,
  ArrowRight as ArrowRightStill,
  ArrowUp as ArrowUpStill,
  ArrowUpDown as ArrowUpDownStill,
  ArrowUpRight as ArrowUpRightStill,
  AudioLines as AudioLinesStill,
  Blocks as BlocksStill,
  Bluetooth as BluetoothStill,
  BluetoothOff as BluetoothOffStill,
  Bold as BoldStill,
  BookA as BookAStill,
  BookPlus as BookPlusStill,
  Bot as BotStill,
  Braces as BracesStill,
  Bug as BugStill,
  CalendarDays as CalendarDaysStill,
  CalendarPlus as CalendarPlusStill,
  Camera as CameraStill,
  Captions as CaptionsStill,
  BarChart3 as BarChart3Still,
  Check as CheckStill,
  CheckCheck as CheckCheckStill,
  ChevronDown as ChevronDownStill,
  ChevronLeft as ChevronLeftStill,
  ChevronRight as ChevronRightStill,
  ChevronUp as ChevronUpStill,
  CircleHelp as CircleHelpStill,
  ClipboardPaste as ClipboardPasteStill,
  Cloud as CloudStill,
  Code as CodeStill,
  Copy as CopyStill,
  CornerDownLeft as CornerDownLeftStill,
  Cpu as CpuStill,
  Crosshair as CrosshairStill,
  Download as DownloadStill,
  Ellipsis as EllipsisStill,
  EllipsisVertical as EllipsisVerticalStill,
  Eraser as EraserStill,
  ExternalLink as ExternalLinkStill,
  Eye as EyeStill,
  EyeOff as EyeOffStill,
  FileDown as FileDownStill,
  FileText as FileTextStill,
  FlaskConical as FlaskConicalStill,
  Folder as FolderStill,
  FolderInput as FolderInputStill,
  FolderPlus as FolderPlusStill,
  Globe as GlobeStill,
  Heading2 as Heading2Still,
  History as HistoryStill,
  Home as HomeStill,
  Info as InfoStill,
  Italic as ItalicStill,
  KeyRound as KeyRoundStill,
  Keyboard as KeyboardStill,
  Languages as LanguagesStill,
  List as ListStill,
  ListChecks as ListChecksStill,
  ListOrdered as ListOrderedStill,
  Mail as MailStill,
  Maximize2 as Maximize2Still,
  Merge as MergeStill,
  MessageCircle as MessageCircleStill,
  Mic as MicStill,
  MicOff as MicOffStill,
  Minimize2 as Minimize2Still,
  Minus as MinusStill,
  NotebookPen as NotebookPenStill,
  PanelLeftClose as PanelLeftCloseStill,
  PanelLeftOpen as PanelLeftOpenStill,
  Pause as PauseStill,
  Pencil as PencilStill,
  PencilLine as PencilLineStill,
  PhoneIncoming as PhoneIncomingStill,
  Pin as PinStill,
  PinOff as PinOffStill,
  Play as PlayStill,
  Plug as PlugStill,
  Plus as PlusStill,
  RefreshCw as RefreshCwStill,
  Replace as ReplaceStill,
  RotateCcw as RotateCcwStill,
  RotateCw as RotateCwStill,
  Scissors as ScissorsStill,
  Search as SearchStill,
  Send as SendStill,
  SendHorizonal as SendHorizonalStill,
  Settings as SettingsStill,
  Settings2 as Settings2Still,
  Shield as ShieldStill,
  ShieldAlert as ShieldAlertStill,
  ShieldCheck as ShieldCheckStill,
  Smartphone as SmartphoneStill,
  SpellCheck as SpellCheckStill,
  Square as SquareStill,
  Edit as EditStill,
  Star as StarStill,
  Tag as TagStill,
  Trash2 as Trash2Still,
  AlertTriangle as AlertTriangleStill,
  Type as TypeStill,
  Undo2 as Undo2Still,
  Upload as UploadStill,
  Users as UsersStill,
  Video as VideoStill,
  Wifi as WifiStill,
  X as XStill,
} from "lucide-react";
import { crafted, hover } from "./components/icons/animated/hover";

export * from "lucide-react";

const ArrowDownMoving = hover(ArrowDownStill, () =>
  import("./components/icons/animated/set/arrow-down").then(
    (m) => m.ArrowDownIcon,
  ),
);
const ArrowDownLeftMoving = hover(ArrowDownLeftStill, () =>
  import("./components/icons/animated/set/arrow-down-left").then(
    (m) => m.ArrowDownLeftIcon,
  ),
);
const ArrowDownRightMoving = hover(ArrowDownRightStill, () =>
  import("./components/icons/animated/set/arrow-down-right").then(
    (m) => m.ArrowDownRightIcon,
  ),
);
const ArrowLeftRightMoving = hover(
  ArrowLeftRightStill,
  crafted(
    "arrow-left-right",
    () => import("lucide-react/dist/esm/icons/arrow-left-right"),
  ),
);
const ArrowRightMoving = hover(ArrowRightStill, () =>
  import("./components/icons/animated/set/arrow-right").then(
    (m) => m.ArrowRightIcon,
  ),
);
const ArrowUpMoving = hover(ArrowUpStill, () =>
  import("./components/icons/animated/set/arrow-up").then((m) => m.ArrowUpIcon),
);
const ArrowUpDownMoving = hover(
  ArrowUpDownStill,
  crafted(
    "arrow-up-down",
    () => import("lucide-react/dist/esm/icons/arrow-up-down"),
  ),
);
const ArrowUpRightMoving = hover(ArrowUpRightStill, () =>
  import("./components/icons/animated/set/arrow-up-right").then(
    (m) => m.ArrowUpRightIcon,
  ),
);
const AudioLinesMoving = hover(AudioLinesStill, () =>
  import("./components/icons/animated/set/audio-lines").then(
    (m) => m.AudioLinesIcon,
  ),
);
const BlocksMoving = hover(BlocksStill, () =>
  import("./components/icons/animated/set/blocks").then((m) => m.BlocksIcon),
);
const BluetoothMoving = hover(BluetoothStill, () =>
  import("./components/icons/animated/set/bluetooth").then(
    (m) => m.BluetoothIcon,
  ),
);
const BluetoothOffMoving = hover(BluetoothOffStill, () =>
  import("./components/icons/animated/set/bluetooth-off").then(
    (m) => m.BluetoothOffIcon,
  ),
);
const BoldMoving = hover(BoldStill, () =>
  import("./components/icons/animated/set/bold").then((m) => m.BoldIcon),
);
const BookAMoving = hover(
  BookAStill,
  crafted("book-a", () => import("lucide-react/dist/esm/icons/book-a")),
);
const BookPlusMoving = hover(
  BookPlusStill,
  crafted("book-plus", () => import("lucide-react/dist/esm/icons/book-plus")),
);
const BotMoving = hover(BotStill, () =>
  import("./components/icons/animated/set/bot").then((m) => m.BotIcon),
);
const BracesMoving = hover(
  BracesStill,
  crafted("braces", () => import("lucide-react/dist/esm/icons/braces")),
);
const BugMoving = hover(
  BugStill,
  crafted("bug", () => import("lucide-react/dist/esm/icons/bug")),
);
const CalendarDaysMoving = hover(CalendarDaysStill, () =>
  import("./components/icons/animated/set/calendar-days").then(
    (m) => m.CalendarDaysIcon,
  ),
);
const CalendarPlusMoving = hover(
  CalendarPlusStill,
  crafted(
    "calendar-plus",
    () => import("lucide-react/dist/esm/icons/calendar-plus"),
  ),
);
const CameraMoving = hover(
  CameraStill,
  crafted("camera", () => import("lucide-react/dist/esm/icons/camera")),
);
const CaptionsMoving = hover(
  CaptionsStill,
  crafted("captions", () => import("lucide-react/dist/esm/icons/captions")),
);
const BarChart3Moving = hover(
  BarChart3Still,
  crafted(
    "chart-column",
    () => import("lucide-react/dist/esm/icons/chart-column"),
  ),
);
const CheckMoving = hover(CheckStill, () =>
  import("./components/icons/animated/set/check").then((m) => m.CheckIcon),
);
const CheckCheckMoving = hover(CheckCheckStill, () =>
  import("./components/icons/animated/set/check-check").then(
    (m) => m.CheckCheckIcon,
  ),
);
const ChevronDownMoving = hover(ChevronDownStill, () =>
  import("./components/icons/animated/set/chevron-down").then(
    (m) => m.ChevronDownIcon,
  ),
);
const ChevronLeftMoving = hover(ChevronLeftStill, () =>
  import("./components/icons/animated/set/chevron-left").then(
    (m) => m.ChevronLeftIcon,
  ),
);
const ChevronRightMoving = hover(ChevronRightStill, () =>
  import("./components/icons/animated/set/chevron-right").then(
    (m) => m.ChevronRightIcon,
  ),
);
const ChevronUpMoving = hover(ChevronUpStill, () =>
  import("./components/icons/animated/set/chevron-up").then(
    (m) => m.ChevronUpIcon,
  ),
);
const CircleHelpMoving = hover(CircleHelpStill, () =>
  import("./components/icons/animated/set/circle-question-mark").then(
    (m) => m.CircleHelpIcon,
  ),
);
const ClipboardPasteMoving = hover(
  ClipboardPasteStill,
  crafted(
    "clipboard-paste",
    () => import("lucide-react/dist/esm/icons/clipboard-paste"),
  ),
);
const CloudMoving = hover(
  CloudStill,
  crafted("cloud", () => import("lucide-react/dist/esm/icons/cloud")),
);
const CodeMoving = hover(
  CodeStill,
  crafted("code", () => import("lucide-react/dist/esm/icons/code")),
);
const CopyMoving = hover(CopyStill, () =>
  import("./components/icons/animated/set/copy").then((m) => m.CopyIcon),
);
const CornerDownLeftMoving = hover(CornerDownLeftStill, () =>
  import("./components/icons/animated/set/corner-down-left").then(
    (m) => m.CornerDownLeftIcon,
  ),
);
const CpuMoving = hover(CpuStill, () =>
  import("./components/icons/animated/set/cpu").then((m) => m.CpuIcon),
);
const CrosshairMoving = hover(
  CrosshairStill,
  crafted("crosshair", () => import("lucide-react/dist/esm/icons/crosshair")),
);
const DownloadMoving = hover(DownloadStill, () =>
  import("./components/icons/animated/set/download").then(
    (m) => m.DownloadIcon,
  ),
);
const EllipsisMoving = hover(
  EllipsisStill,
  crafted("ellipsis", () => import("lucide-react/dist/esm/icons/ellipsis")),
);
const EllipsisVerticalMoving = hover(
  EllipsisVerticalStill,
  crafted(
    "ellipsis-vertical",
    () => import("lucide-react/dist/esm/icons/ellipsis-vertical"),
  ),
);
const EraserMoving = hover(
  EraserStill,
  crafted("eraser", () => import("lucide-react/dist/esm/icons/eraser")),
);
const ExternalLinkMoving = hover(ExternalLinkStill, () =>
  import("./components/icons/animated/set/external-link").then(
    (m) => m.ExternalLinkIcon,
  ),
);
const EyeMoving = hover(EyeStill, () =>
  import("./components/icons/animated/set/eye").then((m) => m.EyeIcon),
);
const EyeOffMoving = hover(EyeOffStill, () =>
  import("./components/icons/animated/set/eye-off").then((m) => m.EyeOffIcon),
);
const FileDownMoving = hover(
  FileDownStill,
  crafted("file-down", () => import("lucide-react/dist/esm/icons/file-down")),
);
const FileTextMoving = hover(FileTextStill, () =>
  import("./components/icons/animated/set/file-text").then(
    (m) => m.FileTextIcon,
  ),
);
const FlaskConicalMoving = hover(
  FlaskConicalStill,
  crafted(
    "flask-conical",
    () => import("lucide-react/dist/esm/icons/flask-conical"),
  ),
);
const FolderMoving = hover(
  FolderStill,
  crafted("folder", () => import("lucide-react/dist/esm/icons/folder")),
);
const FolderInputMoving = hover(FolderInputStill, () =>
  import("./components/icons/animated/set/folder-input").then(
    (m) => m.FolderInputIcon,
  ),
);
const FolderPlusMoving = hover(FolderPlusStill, () =>
  import("./components/icons/animated/set/folder-plus").then(
    (m) => m.FolderPlusIcon,
  ),
);
const GlobeMoving = hover(
  GlobeStill,
  crafted("globe", () => import("lucide-react/dist/esm/icons/globe")),
);
const Heading2Moving = hover(
  Heading2Still,
  crafted("heading-2", () => import("lucide-react/dist/esm/icons/heading-2")),
);
const HistoryMoving = hover(HistoryStill, () =>
  import("./components/icons/animated/set/history").then((m) => m.HistoryIcon),
);
const HomeMoving = hover(HomeStill, () =>
  import("./components/icons/animated/set/house").then((m) => m.HomeIcon),
);
const InfoMoving = hover(
  InfoStill,
  crafted("info", () => import("lucide-react/dist/esm/icons/info")),
);
const ItalicMoving = hover(ItalicStill, () =>
  import("./components/icons/animated/set/italic").then((m) => m.ItalicIcon),
);
const KeyRoundMoving = hover(
  KeyRoundStill,
  crafted("key-round", () => import("lucide-react/dist/esm/icons/key-round")),
);
const KeyboardMoving = hover(KeyboardStill, () =>
  import("./components/icons/animated/set/keyboard").then(
    (m) => m.KeyboardIcon,
  ),
);
const LanguagesMoving = hover(LanguagesStill, () =>
  import("./components/icons/animated/set/languages").then(
    (m) => m.LanguagesIcon,
  ),
);
const ListMoving = hover(
  ListStill,
  crafted("list", () => import("lucide-react/dist/esm/icons/list")),
);
const ListChecksMoving = hover(
  ListChecksStill,
  crafted(
    "list-checks",
    () => import("lucide-react/dist/esm/icons/list-checks"),
  ),
);
const ListOrderedMoving = hover(
  ListOrderedStill,
  crafted(
    "list-ordered",
    () => import("lucide-react/dist/esm/icons/list-ordered"),
  ),
);
const MailMoving = hover(
  MailStill,
  crafted("mail", () => import("lucide-react/dist/esm/icons/mail")),
);
const Maximize2Moving = hover(Maximize2Still, () =>
  import("./components/icons/animated/set/maximize-2").then(
    (m) => m.Maximize2Icon,
  ),
);
const MergeMoving = hover(
  MergeStill,
  crafted("merge", () => import("lucide-react/dist/esm/icons/merge")),
);
const MessageCircleMoving = hover(MessageCircleStill, () =>
  import("./components/icons/animated/set/message-circle").then(
    (m) => m.MessageCircleIcon,
  ),
);
const MicMoving = hover(MicStill, () =>
  import("./components/icons/animated/set/mic").then((m) => m.MicIcon),
);
const MicOffMoving = hover(MicOffStill, () =>
  import("./components/icons/animated/set/mic-off").then((m) => m.MicOffIcon),
);
const Minimize2Moving = hover(
  Minimize2Still,
  crafted("minimize-2", () => import("lucide-react/dist/esm/icons/minimize-2")),
);
const MinusMoving = hover(
  MinusStill,
  crafted("minus", () => import("lucide-react/dist/esm/icons/minus")),
);
const NotebookPenMoving = hover(
  NotebookPenStill,
  crafted(
    "notebook-pen",
    () => import("lucide-react/dist/esm/icons/notebook-pen"),
  ),
);
const PanelLeftCloseMoving = hover(PanelLeftCloseStill, () =>
  import("./components/icons/animated/set/panel-left-close").then(
    (m) => m.PanelLeftCloseIcon,
  ),
);
const PanelLeftOpenMoving = hover(PanelLeftOpenStill, () =>
  import("./components/icons/animated/set/panel-left-open").then(
    (m) => m.PanelLeftOpenIcon,
  ),
);
const PauseMoving = hover(PauseStill, () =>
  import("./components/icons/animated/set/pause").then((m) => m.PauseIcon),
);
const PencilMoving = hover(
  PencilStill,
  crafted("pencil", () => import("lucide-react/dist/esm/icons/pencil")),
);
const PencilLineMoving = hover(
  PencilLineStill,
  crafted(
    "pencil-line",
    () => import("lucide-react/dist/esm/icons/pencil-line"),
  ),
);
const PhoneIncomingMoving = hover(PhoneIncomingStill, () =>
  import("./components/icons/animated/set/phone-incoming").then(
    (m) => m.PhoneIncomingIcon,
  ),
);
const PinMoving = hover(
  PinStill,
  crafted("pin", () => import("lucide-react/dist/esm/icons/pin")),
);
const PinOffMoving = hover(
  PinOffStill,
  crafted("pin-off", () => import("lucide-react/dist/esm/icons/pin-off")),
);
const PlayMoving = hover(PlayStill, () =>
  import("./components/icons/animated/set/play").then((m) => m.PlayIcon),
);
const PlugMoving = hover(
  PlugStill,
  crafted("plug", () => import("lucide-react/dist/esm/icons/plug")),
);
const PlusMoving = hover(PlusStill, () =>
  import("./components/icons/animated/set/plus").then((m) => m.PlusIcon),
);
const RefreshCwMoving = hover(RefreshCwStill, () =>
  import("./components/icons/animated/set/refresh-cw").then(
    (m) => m.RefreshCWIcon,
  ),
);
const ReplaceMoving = hover(
  ReplaceStill,
  crafted("replace", () => import("lucide-react/dist/esm/icons/replace")),
);
const RotateCcwMoving = hover(RotateCcwStill, () =>
  import("./components/icons/animated/set/rotate-ccw").then(
    (m) => m.RotateCCWIcon,
  ),
);
const RotateCwMoving = hover(RotateCwStill, () =>
  import("./components/icons/animated/set/rotate-cw").then(
    (m) => m.RotateCWIcon,
  ),
);
const ScissorsMoving = hover(
  ScissorsStill,
  crafted("scissors", () => import("lucide-react/dist/esm/icons/scissors")),
);
const SearchMoving = hover(SearchStill, () =>
  import("./components/icons/animated/set/search").then((m) => m.SearchIcon),
);
const SendMoving = hover(SendStill, () =>
  import("./components/icons/animated/set/send").then((m) => m.SendIcon),
);
const SendHorizonalMoving = hover(
  SendHorizonalStill,
  crafted(
    "send-horizontal",
    () => import("lucide-react/dist/esm/icons/send-horizontal"),
  ),
);
const SettingsMoving = hover(SettingsStill, () =>
  import("./components/icons/animated/set/settings").then(
    (m) => m.SettingsIcon,
  ),
);
const Settings2Moving = hover(
  Settings2Still,
  crafted("settings-2", () => import("lucide-react/dist/esm/icons/settings-2")),
);
const ShieldMoving = hover(
  ShieldStill,
  crafted("shield", () => import("lucide-react/dist/esm/icons/shield")),
);
const ShieldAlertMoving = hover(
  ShieldAlertStill,
  crafted(
    "shield-alert",
    () => import("lucide-react/dist/esm/icons/shield-alert"),
  ),
);
const ShieldCheckMoving = hover(ShieldCheckStill, () =>
  import("./components/icons/animated/set/shield-check").then(
    (m) => m.ShieldCheckIcon,
  ),
);
const SmartphoneMoving = hover(
  SmartphoneStill,
  crafted("smartphone", () => import("lucide-react/dist/esm/icons/smartphone")),
);
const SpellCheckMoving = hover(
  SpellCheckStill,
  crafted(
    "spell-check",
    () => import("lucide-react/dist/esm/icons/spell-check"),
  ),
);
const SquareMoving = hover(
  SquareStill,
  crafted("square", () => import("lucide-react/dist/esm/icons/square")),
);
const EditMoving = hover(EditStill, () =>
  import("./components/icons/animated/set/square-pen").then(
    (m) => m.SquarePenIcon,
  ),
);
const StarMoving = hover(
  StarStill,
  crafted("star", () => import("lucide-react/dist/esm/icons/star")),
);
const TagMoving = hover(
  TagStill,
  crafted("tag", () => import("lucide-react/dist/esm/icons/tag")),
);
const Trash2Moving = hover(Trash2Still, () =>
  import("./components/icons/animated/set/trash-2").then((m) => m.DeleteIcon),
);
const AlertTriangleMoving = hover(
  AlertTriangleStill,
  crafted(
    "triangle-alert",
    () => import("lucide-react/dist/esm/icons/triangle-alert"),
  ),
);
const TypeMoving = hover(
  TypeStill,
  crafted("type", () => import("lucide-react/dist/esm/icons/type")),
);
const Undo2Moving = hover(
  Undo2Still,
  crafted("undo-2", () => import("lucide-react/dist/esm/icons/undo-2")),
);
const UploadMoving = hover(UploadStill, () =>
  import("./components/icons/animated/set/upload").then((m) => m.UploadIcon),
);
const UsersMoving = hover(UsersStill, () =>
  import("./components/icons/animated/set/users").then((m) => m.UsersIcon),
);
const VideoMoving = hover(
  VideoStill,
  crafted("video", () => import("lucide-react/dist/esm/icons/video")),
);
const WifiMoving = hover(WifiStill, () =>
  import("./components/icons/animated/set/wifi").then((m) => m.WifiIcon),
);
const XMoving = hover(XStill, () =>
  import("./components/icons/animated/set/x").then((m) => m.XIcon),
);

export {
  ArrowDownMoving as ArrowDown,
  ArrowDownMoving as ArrowDownIcon,
  ArrowDownLeftMoving as ArrowDownLeft,
  ArrowDownLeftMoving as ArrowDownLeftIcon,
  ArrowDownRightMoving as ArrowDownRight,
  ArrowDownRightMoving as ArrowDownRightIcon,
  ArrowLeftRightMoving as ArrowLeftRight,
  ArrowLeftRightMoving as ArrowLeftRightIcon,
  ArrowRightMoving as ArrowRight,
  ArrowRightMoving as ArrowRightIcon,
  ArrowUpMoving as ArrowUp,
  ArrowUpMoving as ArrowUpIcon,
  ArrowUpDownMoving as ArrowUpDown,
  ArrowUpDownMoving as ArrowUpDownIcon,
  ArrowUpRightMoving as ArrowUpRight,
  ArrowUpRightMoving as ArrowUpRightIcon,
  AudioLinesMoving as AudioLines,
  AudioLinesMoving as AudioLinesIcon,
  BlocksMoving as Blocks,
  BlocksMoving as BlocksIcon,
  BluetoothMoving as Bluetooth,
  BluetoothMoving as BluetoothIcon,
  BluetoothOffMoving as BluetoothOff,
  BluetoothOffMoving as BluetoothOffIcon,
  BoldMoving as Bold,
  BoldMoving as BoldIcon,
  BookAMoving as BookA,
  BookAMoving as BookAIcon,
  BookPlusMoving as BookPlus,
  BookPlusMoving as BookPlusIcon,
  BotMoving as Bot,
  BotMoving as BotIcon,
  BracesMoving as Braces,
  BracesMoving as BracesIcon,
  BracesMoving as CurlyBraces,
  BracesMoving as CurlyBracesIcon,
  BugMoving as Bug,
  BugMoving as BugIcon,
  CalendarDaysMoving as CalendarDays,
  CalendarDaysMoving as CalendarDaysIcon,
  CalendarPlusMoving as CalendarPlus,
  CalendarPlusMoving as CalendarPlusIcon,
  CameraMoving as Camera,
  CameraMoving as CameraIcon,
  CaptionsMoving as Captions,
  CaptionsMoving as CaptionsIcon,
  CaptionsMoving as Subtitles,
  CaptionsMoving as SubtitlesIcon,
  BarChart3Moving as BarChart3,
  BarChart3Moving as BarChart3Icon,
  BarChart3Moving as ChartColumn,
  BarChart3Moving as ChartColumnIcon,
  CheckMoving as Check,
  CheckMoving as CheckIcon,
  CheckCheckMoving as CheckCheck,
  CheckCheckMoving as CheckCheckIcon,
  ChevronDownMoving as ChevronDown,
  ChevronDownMoving as ChevronDownIcon,
  ChevronLeftMoving as ChevronLeft,
  ChevronLeftMoving as ChevronLeftIcon,
  ChevronRightMoving as ChevronRight,
  ChevronRightMoving as ChevronRightIcon,
  ChevronUpMoving as ChevronUp,
  ChevronUpMoving as ChevronUpIcon,
  CircleHelpMoving as CircleHelp,
  CircleHelpMoving as CircleHelpIcon,
  CircleHelpMoving as CircleQuestionMark,
  CircleHelpMoving as CircleQuestionMarkIcon,
  CircleHelpMoving as HelpCircle,
  CircleHelpMoving as HelpCircleIcon,
  ClipboardPasteMoving as ClipboardPaste,
  ClipboardPasteMoving as ClipboardPasteIcon,
  CloudMoving as Cloud,
  CloudMoving as CloudIcon,
  CodeMoving as Code,
  CodeMoving as CodeIcon,
  CopyMoving as Copy,
  CopyMoving as CopyIcon,
  CornerDownLeftMoving as CornerDownLeft,
  CornerDownLeftMoving as CornerDownLeftIcon,
  CpuMoving as Cpu,
  CpuMoving as CpuIcon,
  CrosshairMoving as Crosshair,
  CrosshairMoving as CrosshairIcon,
  DownloadMoving as Download,
  DownloadMoving as DownloadIcon,
  EllipsisMoving as Ellipsis,
  EllipsisMoving as EllipsisIcon,
  EllipsisMoving as MoreHorizontal,
  EllipsisMoving as MoreHorizontalIcon,
  EllipsisVerticalMoving as EllipsisVertical,
  EllipsisVerticalMoving as EllipsisVerticalIcon,
  EllipsisVerticalMoving as MoreVertical,
  EllipsisVerticalMoving as MoreVerticalIcon,
  EraserMoving as Eraser,
  EraserMoving as EraserIcon,
  ExternalLinkMoving as ExternalLink,
  ExternalLinkMoving as ExternalLinkIcon,
  EyeMoving as Eye,
  EyeMoving as EyeIcon,
  EyeOffMoving as EyeOff,
  EyeOffMoving as EyeOffIcon,
  FileDownMoving as FileDown,
  FileDownMoving as FileDownIcon,
  FileTextMoving as FileText,
  FileTextMoving as FileTextIcon,
  FlaskConicalMoving as FlaskConical,
  FlaskConicalMoving as FlaskConicalIcon,
  FolderMoving as Folder,
  FolderMoving as FolderIcon,
  FolderInputMoving as FolderInput,
  FolderInputMoving as FolderInputIcon,
  FolderPlusMoving as FolderPlus,
  FolderPlusMoving as FolderPlusIcon,
  GlobeMoving as Globe,
  GlobeMoving as GlobeIcon,
  Heading2Moving as Heading2,
  Heading2Moving as Heading2Icon,
  HistoryMoving as History,
  HistoryMoving as HistoryIcon,
  HomeMoving as Home,
  HomeMoving as HomeIcon,
  HomeMoving as House,
  HomeMoving as HouseIcon,
  InfoMoving as Info,
  InfoMoving as InfoIcon,
  ItalicMoving as Italic,
  ItalicMoving as ItalicIcon,
  KeyRoundMoving as KeyRound,
  KeyRoundMoving as KeyRoundIcon,
  KeyboardMoving as Keyboard,
  KeyboardMoving as KeyboardIcon,
  LanguagesMoving as Languages,
  LanguagesMoving as LanguagesIcon,
  ListMoving as List,
  ListMoving as ListIcon,
  ListChecksMoving as ListChecks,
  ListChecksMoving as ListChecksIcon,
  ListOrderedMoving as ListOrdered,
  ListOrderedMoving as ListOrderedIcon,
  MailMoving as Mail,
  MailMoving as MailIcon,
  Maximize2Moving as Maximize2,
  Maximize2Moving as Maximize2Icon,
  MergeMoving as Merge,
  MergeMoving as MergeIcon,
  MessageCircleMoving as MessageCircle,
  MessageCircleMoving as MessageCircleIcon,
  MicMoving as Mic,
  MicMoving as MicIcon,
  MicOffMoving as MicOff,
  MicOffMoving as MicOffIcon,
  Minimize2Moving as Minimize2,
  Minimize2Moving as Minimize2Icon,
  MinusMoving as Minus,
  MinusMoving as MinusIcon,
  NotebookPenMoving as NotebookPen,
  NotebookPenMoving as NotebookPenIcon,
  PanelLeftCloseMoving as PanelLeftClose,
  PanelLeftCloseMoving as PanelLeftCloseIcon,
  PanelLeftCloseMoving as SidebarClose,
  PanelLeftCloseMoving as SidebarCloseIcon,
  PanelLeftOpenMoving as PanelLeftOpen,
  PanelLeftOpenMoving as PanelLeftOpenIcon,
  PanelLeftOpenMoving as SidebarOpen,
  PanelLeftOpenMoving as SidebarOpenIcon,
  PauseMoving as Pause,
  PauseMoving as PauseIcon,
  PencilMoving as Pencil,
  PencilMoving as PencilIcon,
  PencilLineMoving as PencilLine,
  PencilLineMoving as PencilLineIcon,
  PhoneIncomingMoving as PhoneIncoming,
  PhoneIncomingMoving as PhoneIncomingIcon,
  PinMoving as Pin,
  PinMoving as PinIcon,
  PinOffMoving as PinOff,
  PinOffMoving as PinOffIcon,
  PlayMoving as Play,
  PlayMoving as PlayIcon,
  PlugMoving as Plug,
  PlugMoving as PlugIcon,
  PlusMoving as Plus,
  PlusMoving as PlusIcon,
  RefreshCwMoving as RefreshCw,
  RefreshCwMoving as RefreshCwIcon,
  ReplaceMoving as Replace,
  ReplaceMoving as ReplaceIcon,
  RotateCcwMoving as RotateCcw,
  RotateCcwMoving as RotateCcwIcon,
  RotateCwMoving as RotateCw,
  RotateCwMoving as RotateCwIcon,
  ScissorsMoving as Scissors,
  ScissorsMoving as ScissorsIcon,
  SearchMoving as Search,
  SearchMoving as SearchIcon,
  SendMoving as Send,
  SendMoving as SendIcon,
  SendHorizonalMoving as SendHorizonal,
  SendHorizonalMoving as SendHorizonalIcon,
  SendHorizonalMoving as SendHorizontal,
  SendHorizonalMoving as SendHorizontalIcon,
  SettingsMoving as Settings,
  SettingsMoving as SettingsIcon,
  Settings2Moving as Settings2,
  Settings2Moving as Settings2Icon,
  ShieldMoving as Shield,
  ShieldMoving as ShieldIcon,
  ShieldAlertMoving as ShieldAlert,
  ShieldAlertMoving as ShieldAlertIcon,
  ShieldCheckMoving as ShieldCheck,
  ShieldCheckMoving as ShieldCheckIcon,
  SmartphoneMoving as Smartphone,
  SmartphoneMoving as SmartphoneIcon,
  SpellCheckMoving as SpellCheck,
  SpellCheckMoving as SpellCheckIcon,
  SquareMoving as Square,
  SquareMoving as SquareIcon,
  EditMoving as Edit,
  EditMoving as EditIcon,
  EditMoving as PenBox,
  EditMoving as PenBoxIcon,
  EditMoving as PenSquare,
  EditMoving as PenSquareIcon,
  EditMoving as SquarePen,
  EditMoving as SquarePenIcon,
  StarMoving as Star,
  StarMoving as StarIcon,
  TagMoving as Tag,
  TagMoving as TagIcon,
  Trash2Moving as Trash2,
  Trash2Moving as Trash2Icon,
  AlertTriangleMoving as AlertTriangle,
  AlertTriangleMoving as AlertTriangleIcon,
  AlertTriangleMoving as TriangleAlert,
  AlertTriangleMoving as TriangleAlertIcon,
  TypeMoving as Type,
  TypeMoving as TypeIcon,
  Undo2Moving as Undo2,
  Undo2Moving as Undo2Icon,
  UploadMoving as Upload,
  UploadMoving as UploadIcon,
  UsersMoving as Users,
  UsersMoving as UsersIcon,
  VideoMoving as Video,
  VideoMoving as VideoIcon,
  WifiMoving as Wifi,
  WifiMoving as WifiIcon,
  XMoving as X,
  XMoving as XIcon,
};
