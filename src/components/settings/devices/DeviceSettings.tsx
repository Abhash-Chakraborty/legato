import React, { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  ArrowDownLeft,
  ArrowUpRight,
  Check,
  Copy,
  Network,
  RefreshCw,
  Smartphone,
  Wifi,
  X,
} from "lucide-react";
import {
  commands,
  type ClipRow,
  type Merge,
  type PairingInfo,
  type SyncSettings,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useSyncStore, watchSync } from "../../../stores/syncStore";
import { formatRelativeTime } from "../../../utils/dateFormat";
import { ActionMenu } from "../../ui/ActionMenu";
import { Button } from "../../ui/Button";
import { Dialog } from "../../ui/Dialog";
import { Dropdown } from "../../ui/Dropdown";
import { SettingContainer } from "../../ui/SettingContainer";
import { ToggleSwitch } from "../../ui/ToggleSwitch";

export const SYNC_KINDS = [
  "dictionary",
  "snippets",
  "history",
  "notes",
  "scratchpad",
  "transforms",
  "style",
] as const;
export type SyncKind = (typeof SYNC_KINDS)[number];

/** Rust's i64::MAX: paused until turned back on. */
const PAUSED_UNTIL_RESUMED = 9223372036854775807;

const DEFAULTS: Required<SyncSettings> = {
  kinds: [...SYNC_KINDS],
  pc_name: null,
  clipboard: true,
  clipboard_images_mb: 10,
  clipboard_never: ["KeePassXC.exe", "Bitwarden.exe", "1Password.exe"],
  clipboard_paused_until: null,
  phone_pill: false,
};

/** Settings › Devices, with the defaults filled in, and a setter for one
 *  field. */
export const useSyncSettings = () => {
  const { settings, updateSetting, isUpdating } = useSettings();
  const sync = { ...DEFAULTS, ...settings?.sync };
  const set = <K extends keyof SyncSettings>(key: K, value: SyncSettings[K]) =>
    updateSetting("sync", { ...sync, [key]: value });
  return { sync, set, busy: isUpdating("sync") };
};

/** The paired phone, if there is one, kept current. */
export const usePhone = () => {
  useEffect(watchSync, []);
  const state = useSyncStore((s) => s.state);
  return { state, phone: state?.phones[0] ?? null };
};

const nowS = () => Math.floor(Date.now() / 1000);

export const clipboardPaused = (sync: Required<SyncSettings>) =>
  sync.clipboard_paused_until != null && sync.clipboard_paused_until > nowS();

/* -- pairing ---------------------------------------------------------------- */

const PairDialog: React.FC<{ open: boolean; onClose: () => void }> = ({
  open,
  onClose,
}) => {
  const { t } = useTranslation();
  const state = useSyncStore((s) => s.state);
  const [info, setInfo] = useState<PairingInfo | null>(null);
  const [error, setError] = useState<string | null>(null);
  const known = useRef<string[]>([]);
  const phones = state?.phones ?? [];
  const paired = phones.find(
    (p) => !known.current.includes(p.peer.device_id),
  )?.peer;

  const start = async () => {
    setError(null);
    known.current = (useSyncStore.getState().state?.phones ?? []).map(
      (p) => p.peer.device_id,
    );
    const result = await commands.startPairing();
    if (result.status === "ok") setInfo(result.data);
    else setError(result.error);
  };

  useEffect(() => {
    if (open) start();
    else setInfo(null);
  }, [open]);

  const close = () => {
    if (!paired) commands.cancelPairing();
    onClose();
  };

  const expired = !!info && !paired && state != null && !state.pairing;

  const copyLink = async () => {
    if (!info) return;
    await navigator.clipboard.writeText(info.link);
    toast.success(t("settings.devices.pair.copied"));
  };

  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && close()}
      title={
        paired
          ? t("settings.devices.pair.done", { name: paired.name })
          : t("settings.devices.pair.title")
      }
      description={
        paired
          ? undefined
          : t("settings.devices.pair.subtitle", {
              pc: info?.pc_name ?? state?.pc_name ?? "",
            })
      }
      closeLabel={t("settings.devices.pair.cancel")}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant={paired ? "primary" : "ghost"} onClick={close}>
            {paired
              ? t("settings.devices.pair.finish")
              : t("settings.devices.pair.cancel")}
          </Button>
        </div>
      }
    >
      {paired ? (
        <div role="status" className="grid justify-items-center gap-2 py-6">
          <span className="grid h-11 w-11 place-items-center rounded-full bg-teal-fill text-white">
            <Check className="h-5 w-5 stroke-[2.5]" aria-hidden="true" />
          </span>
          <p className="max-w-[360px] text-center text-sm text-text-2">
            {t("settings.devices.pair.firstSync")}
          </p>
        </div>
      ) : error ? (
        <p className="py-4 text-sm text-danger">{error}</p>
      ) : (
        <div className="grid gap-5">
          <div className="flex items-start gap-5">
            <div className="grid h-[200px] w-[200px] flex-none place-items-center rounded-[var(--r-md)] border border-line bg-white p-2">
              {info && !expired ? (
                <img
                  src={`data:image/svg+xml;utf8,${encodeURIComponent(info.qr_svg)}`}
                  alt={t("settings.devices.pair.qr", { pc: info.pc_name })}
                  className="h-full w-full [image-rendering:pixelated]"
                />
              ) : (
                <span className="text-[13px] text-neutral-500">
                  {expired
                    ? t("settings.devices.pair.expired")
                    : t("settings.devices.pair.making")}
                </span>
              )}
            </div>
            <ol className="grid gap-2.5 text-sm">
              {(["open", "scan", "confirm"] as const).map((step, i) => (
                <li key={step} className="flex gap-2.5">
                  <span
                    aria-hidden="true"
                    className="grid h-5 w-5 flex-none place-items-center rounded-full bg-surface-3 text-xs font-medium"
                  >
                    {i + 1}
                  </span>
                  <span>
                    {t(`settings.devices.pair.steps.${step}`, {
                      pc: info?.pc_name ?? "",
                    })}
                  </span>
                </li>
              ))}
              <li className="mt-1 text-[13px] text-text-3">
                {t("settings.devices.pair.once", {
                  count: info?.minutes ?? 10,
                })}
              </li>
              <li className="flex gap-2">
                {expired ? (
                  <Button size="sm" onClick={start}>
                    {t("settings.devices.pair.again")}
                  </Button>
                ) : (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={copyLink}
                    disabled={!info}
                  >
                    <Copy className="h-3.5 w-3.5" aria-hidden="true" />
                    {t("settings.devices.pair.copyLink")}
                  </Button>
                )}
              </li>
            </ol>
          </div>
          <div className="grid gap-1.5 rounded-[var(--r-md)] bg-surface-2 p-3 text-[13px]">
            {(info?.addresses ?? []).map((a) => {
              const Icon = a.kind === "netbird" ? Network : Wifi;
              return (
                <div key={a.addr} className="flex items-center gap-2">
                  <Icon className="h-3.5 w-3.5 text-text-3" aria-hidden />
                  <span className="w-24">
                    {t(`settings.devices.via.${a.kind}`)}
                  </span>
                  <span className="font-mono text-xs text-text-2">
                    {a.addr}
                  </span>
                </div>
              );
            })}
            {info && info.addresses.length === 0 && (
              <span className="text-danger">
                {t("settings.devices.pair.noNetwork")}
              </span>
            )}
            <p className="text-text-3">{t("settings.devices.pair.private")}</p>
          </div>
          {!expired && (
            <div
              aria-live="polite"
              className="flex items-center gap-2 text-[13px] text-text-2"
            >
              <span
                aria-hidden="true"
                className="h-2 w-2 animate-pulse rounded-full bg-teal-fill motion-reduce:animate-none"
              />
              {t("settings.devices.pair.waiting")}
            </div>
          )}
        </div>
      )}
    </Dialog>
  );
};

/** With no phone paired, the only thing Devices shows: pair one. */
const PairPhoneRow: React.FC<{ onPair: () => void }> = ({ onPair }) => {
  const { t } = useTranslation();
  const { state } = usePhone();
  return (
    <SettingContainer
      title={t("settings.devices.pairCard.title")}
      description={
        state?.listen_error
          ? t("settings.devices.pairCard.listenError", {
              error: state.listen_error,
            })
          : t("settings.devices.pairCard.help")
      }
      grouped
    >
      <Button onClick={onPair}>
        <Smartphone className="h-4 w-4" aria-hidden="true" />
        {t("settings.devices.pairCard.button")}
      </Button>
    </SettingContainer>
  );
};

/** Pair a phone, or the paired phone. The dialog lives here so it stays
 * open to say the phone was paired when the row underneath switches. */
export const PhoneCard: React.FC = () => {
  const { phone } = usePhone();
  const [open, setOpen] = useState(false);
  return (
    <>
      {phone ? <PhoneRow /> : <PairPhoneRow onPair={() => setOpen(true)} />}
      <PairDialog open={open} onClose={() => setOpen(false)} />
    </>
  );
};

/* -- the paired phone -------------------------------------------------------- */

const NameDialog: React.FC<{
  open: boolean;
  name: string;
  onClose: () => void;
  onSave: (name: string) => void;
}> = ({ open, name, onClose, onSave }) => {
  const { t } = useTranslation();
  const [value, setValue] = useState(name);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (open) setValue(name);
  }, [open, name]);
  const save = () => {
    const v = value.trim();
    if (v) onSave(v);
  };
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={t("settings.devices.rename.title")}
      closeLabel={t("settings.devices.rename.cancel")}
      initialFocusRef={input as React.RefObject<HTMLElement>}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" onClick={onClose}>
            {t("settings.devices.rename.cancel")}
          </Button>
          <Button onClick={save} disabled={!value.trim()}>
            {t("settings.devices.rename.save")}
          </Button>
        </div>
      }
    >
      <form
        onSubmit={(e) => {
          e.preventDefault();
          save();
        }}
      >
        <label className="grid gap-1.5 text-sm">
          {t("settings.devices.rename.label")}
          <input
            ref={input}
            className="input"
            value={value}
            maxLength={40}
            onChange={(e) => setValue(e.target.value)}
          />
        </label>
      </form>
    </Dialog>
  );
};

const PhoneRow: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { phone } = usePhone();
  const [syncing, setSyncing] = useState(false);
  const [renaming, setRenaming] = useState(false);
  const [unpairing, setUnpairing] = useState(false);
  if (!phone) return null;
  const { peer } = phone;
  const paired = new Date(peer.paired_at * 1000).toLocaleDateString(
    i18n.language,
    { month: "short", day: "numeric" },
  );
  const synced = peer.last_sync
    ? formatRelativeTime(String(peer.last_sync), i18n.language)
    : null;
  // Kept as pieces so a narrow row breaks between them, not inside one.
  const status = [
    phone.online
      ? t(`settings.devices.phone.online.${phone.via ?? "lan"}`)
      : t("settings.devices.phone.offline"),
    ...(synced
      ? [
          t(
            phone.online
              ? "settings.devices.phone.synced"
              : "settings.devices.phone.lastSynced",
            { when: synced },
          ),
        ]
      : []),
  ];

  const syncNow = async () => {
    setSyncing(true);
    const result = await commands.syncPhoneNow(peer.device_id);
    setTimeout(() => setSyncing(false), 900);
    if (result.status === "error") toast.error(result.error);
    else if (!result.data)
      toast.warning(
        t("settings.devices.phone.unreachable", { name: peer.name }),
      );
  };

  const rename = async (name: string) => {
    const result = await commands.renamePhone(peer.device_id, name);
    if (result.status === "error") toast.error(result.error);
    setRenaming(false);
  };

  const unpair = async () => {
    const result = await commands.unpairPhone(peer.device_id);
    setUnpairing(false);
    if (result.status === "error") toast.error(result.error);
    else toast(t("settings.devices.unpair.done", { name: peer.name }));
  };

  return (
    <div
      id="settings-devices-phone"
      className="flex flex-wrap items-center gap-x-4 gap-y-3 py-3.5"
    >
      <span className="grid h-10 w-10 flex-none place-items-center rounded-[var(--r-md)] border border-line bg-surface-2">
        <Smartphone className="h-5 w-5 stroke-[1.5]" aria-hidden="true" />
      </span>
      <div className="min-w-0 flex-1">
        <div className="text-[15px] font-medium">{peer.name}</div>
        <div className="text-[13px] text-text-3">
          {t("settings.devices.phone.about", {
            version: peer.app_version || "—",
            date: paired,
          })}
        </div>
        <div className="mt-0.5 flex items-center gap-1.5 text-[13px] text-text-2">
          <span
            aria-hidden="true"
            className={`h-2 w-2 flex-none rounded-full ${phone.online ? "bg-teal-fill" : "border border-text-3"}`}
          />
          <span className="flex flex-wrap gap-x-1">
            {status.map((part, i) => (
              <span key={part} className="whitespace-nowrap">
                {part}
                {i < status.length - 1 && " ·"}
              </span>
            ))}
          </span>
        </div>
      </div>
      <div className="flex items-center gap-1">
        <Button
          variant="secondary"
          size="sm"
          onClick={syncNow}
          disabled={syncing}
        >
          <RefreshCw
            className={`h-3.5 w-3.5 ${syncing ? "animate-spin motion-reduce:animate-none" : ""}`}
            aria-hidden="true"
          />
          {t("settings.devices.phone.syncNow")}
        </Button>
        <Button variant="ghost" size="sm" onClick={() => setRenaming(true)}>
          {t("settings.devices.phone.rename")}
        </Button>
        <Button variant="ghost" size="sm" onClick={() => setUnpairing(true)}>
          {t("settings.devices.phone.unpair")}
        </Button>
      </div>
      <NameDialog
        open={renaming}
        name={peer.name}
        onClose={() => setRenaming(false)}
        onSave={rename}
      />
      <Dialog
        open={unpairing}
        onOpenChange={(next) => !next && setUnpairing(false)}
        title={t("settings.devices.unpair.title", { name: peer.name })}
        description={t("settings.devices.unpair.help", { name: peer.name })}
        closeLabel={t("settings.devices.unpair.cancel")}
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setUnpairing(false)}>
              {t("settings.devices.unpair.cancel")}
            </Button>
            <Button variant="danger" onClick={unpair}>
              {t("settings.devices.unpair.confirm")}
            </Button>
          </div>
        }
      >
        {null}
      </Dialog>
    </div>
  );
};

const MergesDialog: React.FC<{ open: boolean; onClose: () => void }> = ({
  open,
  onClose,
}) => {
  const { t, i18n } = useTranslation();
  const [merges, setMerges] = useState<Merge[]>([]);
  useEffect(() => {
    if (!open) return;
    commands
      .getSyncMerges()
      .then((r) => r.status === "ok" && setMerges(r.data));
  }, [open]);
  return (
    <Dialog
      open={open}
      onOpenChange={(next) => !next && onClose()}
      title={t("settings.devices.merges.title")}
      description={t("settings.devices.merges.help")}
      closeLabel={t("settings.devices.merges.done")}
      footer={
        <div className="flex justify-end">
          <Button onClick={onClose}>{t("settings.devices.merges.done")}</Button>
        </div>
      }
    >
      <ul className="divide-y divide-line">
        {merges.map((m) => (
          <li key={m.id} className="py-2.5">
            <div className="text-sm font-medium">
              {t(`settings.devices.kinds.${m.kind}.title`)}
              {m.label ? ` · ${m.label}` : ""}
            </div>
            <div className="text-[13px] text-text-3">
              {new Date(m.at * 1000).toLocaleString(i18n.language, {
                month: "short",
                day: "numeric",
                hour: "numeric",
                minute: "2-digit",
              })}
              {" · "}
              {m.kept_theirs
                ? t("settings.devices.merges.keptTheirs", { name: m.peer })
                : t("settings.devices.merges.keptOurs")}
            </div>
          </li>
        ))}
      </ul>
    </Dialog>
  );
};

export const SyncStatusRow: React.FC = () => {
  const { t } = useTranslation();
  const { state, phone } = usePhone();
  const [open, setOpen] = useState(false);
  if (!phone || !state) return null;
  const merges = state.merges;
  const description = [
    phone.online
      ? t("settings.devices.status.upToDate")
      : t("settings.devices.status.waiting", { name: phone.peer.name }),
    merges > 0 ? t("settings.devices.status.merged", { count: merges }) : "",
  ]
    .filter(Boolean)
    .join(" ");
  return (
    <SettingContainer
      title={t("settings.devices.status.title")}
      description={description}
      grouped
    >
      {merges > 0 && (
        <Button variant="ghost" size="sm" onClick={() => setOpen(true)}>
          {t("settings.devices.status.seeMerges")}
        </Button>
      )}
      <MergesDialog open={open} onClose={() => setOpen(false)} />
    </SettingContainer>
  );
};

/** What this PC is called on the phone. */
export const PcNameRow: React.FC = () => {
  const { t } = useTranslation();
  const { state } = usePhone();
  const { sync, set } = useSyncSettings();
  const [value, setValue] = useState(sync.pc_name ?? "");
  useEffect(() => setValue(sync.pc_name ?? ""), [sync.pc_name]);
  const commit = () => {
    const v = value.trim();
    if (v !== (sync.pc_name ?? "")) set("pc_name", v || null);
  };
  return (
    <SettingContainer
      title={t("settings.devices.pcName.title")}
      description={t("settings.devices.pcName.help")}
      grouped
    >
      <input
        className="input compact w-56"
        value={value}
        maxLength={40}
        placeholder={state?.pc_name}
        aria-label={t("settings.devices.pcName.title")}
        onChange={(e) => setValue(e.target.value)}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") setValue(sync.pc_name ?? "");
        }}
      />
    </SettingContainer>
  );
};

/* -- what syncs ------------------------------------------------------------- */

export const SyncKindRow: React.FC<{ kind: SyncKind }> = ({ kind }) => {
  const { t } = useTranslation();
  const { state } = usePhone();
  const { sync, set, busy } = useSyncSettings();
  const count = state?.counts.find((c) => c.kind === kind)?.count ?? 0;
  const on = sync.kinds.includes(kind);
  return (
    <ToggleSwitch
      checked={on}
      onChange={(next) =>
        set(
          "kinds",
          next
            ? SYNC_KINDS.filter((k) => k === kind || sync.kinds.includes(k))
            : sync.kinds.filter((k) => k !== kind),
        )
      }
      isUpdating={busy}
      label={t(`settings.devices.kinds.${kind}.title`)}
      description={t(`settings.devices.kinds.${kind}.help`, { count })}
      descriptionMode="inline"
      grouped
    />
  );
};

export const NeverSyncsRow: React.FC = () => {
  const { t } = useTranslation();
  return (
    <p data-setting-help className="py-3 text-[13px] text-text-3">
      {t("settings.devices.never")}
    </p>
  );
};

/* -- clipboard -------------------------------------------------------------- */

export const SharedClipboardRow: React.FC = () => {
  const { t } = useTranslation();
  const { sync, set, busy } = useSyncSettings();
  return (
    <ToggleSwitch
      checked={sync.clipboard}
      onChange={(on) => set("clipboard", on)}
      isUpdating={busy}
      label={t("settings.devices.clipboard.title")}
      description={t("settings.devices.clipboard.help")}
      descriptionMode="inline"
      grouped
    />
  );
};

export const ClipboardImagesRow: React.FC = () => {
  const { t } = useTranslation();
  const { sync, set } = useSyncSettings();
  return (
    <SettingContainer
      title={t("settings.devices.images.title")}
      description={t("settings.devices.images.help")}
      grouped
    >
      <Dropdown
        ariaLabel={t("settings.devices.images.title")}
        selectedValue={String(sync.clipboard_images_mb)}
        onSelect={(v) => set("clipboard_images_mb", Number(v))}
        options={["0", "1", "10"].map((mb) => ({
          value: mb,
          label: t(`settings.devices.images.options.${mb}`),
        }))}
      />
    </SettingContainer>
  );
};

export const PrivateContentRow: React.FC = () => {
  const { t } = useTranslation();
  return (
    <SettingContainer
      title={t("settings.devices.private.title")}
      description={t("settings.devices.private.help")}
      grouped
    >
      <span className="text-[13px] text-text-2">
        {t("settings.devices.private.always")}
      </span>
    </SettingContainer>
  );
};

export const NeverShareAppsRow: React.FC = () => {
  const { t } = useTranslation();
  const { sync, set } = useSyncSettings();
  const [adding, setAdding] = useState("");
  const apps = sync.clipboard_never;
  const add = () => {
    let name = adding.trim();
    if (!name) return;
    if (!/\.exe$/i.test(name)) name += ".exe";
    if (!apps.some((a) => a.toLowerCase() === name.toLowerCase()))
      set("clipboard_never", [...apps, name]);
    setAdding("");
  };
  const remove = (app: string) => {
    const before = apps;
    set(
      "clipboard_never",
      apps.filter((a) => a !== app),
    );
    toast(t("settings.devices.neverApps.removed", { app }), {
      action: {
        label: t("settings.devices.undo"),
        onClick: () => set("clipboard_never", before),
      },
    });
  };
  return (
    <div>
      <SettingContainer
        title={t("settings.devices.neverApps.title")}
        description={t("settings.devices.neverApps.help")}
        grouped
      >
        <span className="text-[13px] tabular-nums text-text-2">
          {t("settings.devices.neverApps.count", { count: apps.length })}
        </span>
      </SettingContainer>
      <ul className="ms-6 border-s border-line">
        {apps.map((app) => (
          <li
            key={app}
            className="group flex items-center gap-3 border-b border-line py-2 ps-4"
          >
            <span className="flex-1 font-mono text-[13px]">{app}</span>
            <button
              type="button"
              aria-label={t("settings.devices.neverApps.remove", { app })}
              title={t("settings.devices.neverApps.remove", { app })}
              onClick={() => remove(app)}
              className="grid h-7 w-7 place-items-center rounded-[var(--r-xs)] text-text-3 opacity-0 transition-opacity duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text group-hover:opacity-100 group-hover:duration-0 focus-visible:opacity-100"
            >
              <X className="h-3.5 w-3.5" aria-hidden="true" />
            </button>
          </li>
        ))}
        <li className="flex items-center gap-2 py-2 ps-4">
          <input
            className="input compact w-60"
            value={adding}
            placeholder={t("settings.devices.neverApps.placeholder")}
            aria-label={t("settings.devices.neverApps.add")}
            onChange={(e) => setAdding(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter") add();
              if (e.key === "Escape") setAdding("");
            }}
          />
          <Button
            variant="secondary"
            size="sm"
            onClick={add}
            disabled={!adding.trim()}
          >
            {t("settings.devices.neverApps.add")}
          </Button>
        </li>
      </ul>
    </div>
  );
};

export const PhonePillRow: React.FC = () => {
  const { t } = useTranslation();
  const { phone } = usePhone();
  const { sync, set, busy } = useSyncSettings();
  return (
    <ToggleSwitch
      checked={sync.phone_pill}
      onChange={(on) => set("phone_pill", on)}
      isUpdating={busy}
      label={t("settings.devices.pill.title")}
      description={t("settings.devices.pill.help", {
        name: phone?.peer.name ?? "",
      })}
      descriptionMode="inline"
      grouped
    />
  );
};

export const PauseClipboardRow: React.FC = () => {
  const { t, i18n } = useTranslation();
  const { sync, set } = useSyncSettings();
  const [menu, setMenu] = useState(false);
  const wrap = useRef<HTMLDivElement>(null);
  const paused = clipboardPaused(sync);
  const until = sync.clipboard_paused_until;
  const pausedLabel =
    until === null
      ? ""
      : until >= PAUSED_UNTIL_RESUMED / 2
        ? t("settings.devices.pause.untilResumed")
        : t("settings.devices.pause.until", {
            time: new Date(until * 1000).toLocaleString(i18n.language, {
              weekday: "short",
              hour: "numeric",
              minute: "2-digit",
            }),
          });
  const pause = (value: number | null) => {
    const before = sync.clipboard_paused_until;
    set("clipboard_paused_until", value);
    toast(
      value
        ? t("settings.devices.pause.paused")
        : t("settings.devices.pause.resumed"),
      {
        action: {
          label: t("settings.devices.undo"),
          onClick: () => set("clipboard_paused_until", before),
        },
      },
    );
  };
  const tomorrow = () => {
    const d = new Date();
    d.setDate(d.getDate() + 1);
    d.setHours(8, 0, 0, 0);
    return Math.floor(d.getTime() / 1000);
  };
  return (
    <SettingContainer
      title={t("settings.devices.pause.title")}
      description={paused ? pausedLabel : t("settings.devices.pause.help")}
      grouped
    >
      {paused ? (
        <Button variant="secondary" size="sm" onClick={() => pause(null)}>
          {t("settings.devices.pause.resume")}
        </Button>
      ) : (
        <div ref={wrap} className="relative">
          <Button
            variant="secondary"
            size="sm"
            aria-haspopup="menu"
            aria-expanded={menu}
            onClick={() => setMenu(!menu)}
          >
            {t("settings.devices.pause.button")}
          </Button>
          {menu && (
            <ActionMenu
              label={t("settings.devices.pause.title")}
              className="end-0 top-full z-20 mt-1 w-56"
              onClose={(restore) => {
                setMenu(false);
                if (restore) wrap.current?.querySelector("button")?.focus();
              }}
              items={[
                {
                  id: "hour",
                  label: t("settings.devices.pause.hour"),
                  onSelect: () => pause(nowS() + 3600),
                },
                {
                  id: "tomorrow",
                  label: t("settings.devices.pause.tomorrow"),
                  onSelect: () => pause(tomorrow()),
                },
                {
                  id: "manual",
                  label: t("settings.devices.pause.manual"),
                  onSelect: () => pause(PAUSED_UNTIL_RESUMED),
                },
              ]}
            />
          )}
        </div>
      )}
    </SettingContainer>
  );
};

export const SendClipboardRow: React.FC = () => {
  const { t } = useTranslation();
  const { phone } = usePhone();
  const { sync } = useSyncSettings();
  if (!phone) return null;
  const name = phone.peer.name;
  const send = async () => {
    const result = await commands.sendClipboardToPhone();
    if (result.status === "error") toast.error(result.error);
    else toast.success(t("settings.devices.send.sent", { name }));
  };
  return (
    <SettingContainer
      title={t("settings.devices.send.title")}
      description={
        phone.online
          ? t("settings.devices.send.help")
          : t("settings.devices.send.offline", { name })
      }
      grouped
    >
      <Button
        variant="secondary"
        size="sm"
        onClick={send}
        disabled={!phone.online || clipboardPaused(sync)}
      >
        <ArrowUpRight className="h-3.5 w-3.5" aria-hidden="true" />
        {t("settings.devices.send.button", { name })}
      </Button>
    </SettingContainer>
  );
};

/* -- recent clips ------------------------------------------------------------ */

const ClipThumb: React.FC<{ id: string; alt: string }> = ({ id, alt }) => {
  const [src, setSrc] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    commands.getClipImage(id).then((r) => {
      if (live && r.status === "ok") setSrc(r.data);
    });
    return () => {
      live = false;
    };
  }, [id]);
  return src ? (
    <img
      src={src}
      alt={alt}
      className="max-h-16 max-w-[160px] rounded-[var(--r-xs)] border border-line object-contain"
    />
  ) : (
    <span className="block h-12 w-20 rounded-[var(--r-xs)] bg-surface-3" />
  );
};

const sizeLabel = (bytes: number) =>
  bytes >= 1024 * 1024
    ? `${(bytes / 1024 / 1024).toFixed(1)} MB`
    : `${Math.max(1, Math.round(bytes / 1024))} KB`;

export const RecentClipsRow: React.FC = () => {
  const { t, i18n } = useTranslation();
  const clips = useSyncStore((s) => s.clips);
  const [clearing, setClearing] = useState(false);
  useEffect(watchSync, []);

  const copy = async (clip: ClipRow) => {
    const result = await commands.copyRecentClip(clip.id);
    if (result.status === "error") toast.error(result.error);
    else toast.success(t("settings.devices.clips.copied"));
  };

  const clear = async () => {
    setClearing(false);
    const result = await commands.clearRecentClips();
    if (result.status === "error") toast.error(result.error);
  };

  // Up and Down move between clips; Enter copies one again.
  const onKeyDown = (e: React.KeyboardEvent<HTMLUListElement>) => {
    const rows = Array.from(
      e.currentTarget.querySelectorAll<HTMLLIElement>("li[data-clip]"),
    );
    const i = rows.indexOf(document.activeElement as HTMLLIElement);
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const next =
        rows[
          Math.min(
            rows.length - 1,
            Math.max(0, i + (e.key === "ArrowDown" ? 1 : -1)),
          )
        ];
      next?.focus();
    } else if (e.key === "Enter" && i >= 0) {
      e.preventDefault();
      copy(clips[i]);
    }
  };

  return (
    <div className="py-1">
      <SettingContainer
        title={t("settings.devices.clips.title")}
        description={t("settings.devices.clips.help")}
        grouped
      >
        {clips.length > 0 && (
          <Button variant="ghost" size="sm" onClick={() => setClearing(true)}>
            {t("settings.devices.clips.clear")}
          </Button>
        )}
      </SettingContainer>
      {clips.length === 0 ? (
        <p className="pb-3 text-[13px] text-text-3">
          {t("settings.devices.clips.empty")}
        </p>
      ) : (
        <ul
          className="divide-y divide-line"
          aria-label={t("settings.devices.clips.title")}
          onKeyDown={onKeyDown}
        >
          {clips.map((clip, i) => {
            const Icon = clip.incoming ? ArrowDownLeft : ArrowUpRight;
            const when = new Date(clip.at * 1000).toLocaleTimeString(
              i18n.language,
              { hour: "numeric", minute: "2-digit" },
            );
            const meta = [
              clip.incoming
                ? t("settings.devices.clips.from", { name: clip.peer })
                : t("settings.devices.clips.to", { name: clip.peer }),
              when,
              clip.from_app,
              clip.kind === "image" ? sizeLabel(clip.bytes) : null,
            ]
              .filter(Boolean)
              .join(" · ");
            return (
              <li
                key={clip.id}
                data-clip
                tabIndex={i === 0 ? 0 : -1}
                className="group flex items-start gap-3 py-2.5 ring-inside"
              >
                <Icon
                  className="mt-0.5 h-4 w-4 flex-none text-text-3"
                  aria-label={
                    clip.incoming
                      ? t("settings.devices.clips.incoming")
                      : t("settings.devices.clips.outgoing")
                  }
                />
                <div className="min-w-0 flex-1">
                  {clip.kind === "image" ? (
                    <ClipThumb
                      id={clip.id}
                      alt={t("settings.devices.clips.image")}
                    />
                  ) : (
                    <p className="line-clamp-2 whitespace-pre-wrap break-words text-sm">
                      {clip.text}
                    </p>
                  )}
                  <div className="mt-0.5 text-[12px] text-text-3">{meta}</div>
                  {clip.late && (
                    <div className="mt-0.5 text-[12px] text-warning">
                      {t("settings.devices.clips.late")}
                    </div>
                  )}
                </div>
                <button
                  type="button"
                  tabIndex={-1}
                  aria-label={t("settings.devices.clips.copy")}
                  title={t("settings.devices.clips.copy")}
                  onClick={() => copy(clip)}
                  className="grid h-7 w-7 flex-none place-items-center rounded-[var(--r-xs)] text-text-3 opacity-0 transition-opacity duration-[var(--dur-1)] hover:bg-surface-3 hover:text-text group-hover:opacity-100 group-hover:duration-0 group-focus-within:opacity-100"
                >
                  <Copy className="h-3.5 w-3.5" aria-hidden="true" />
                </button>
              </li>
            );
          })}
        </ul>
      )}
      <Dialog
        open={clearing}
        onOpenChange={(next) => !next && setClearing(false)}
        title={t("settings.devices.clips.clearTitle")}
        description={t("settings.devices.clips.clearHelp")}
        closeLabel={t("settings.devices.clips.cancel")}
        footer={
          <div className="flex justify-end gap-2">
            <Button variant="ghost" onClick={() => setClearing(false)}>
              {t("settings.devices.clips.cancel")}
            </Button>
            <Button variant="danger" onClick={clear}>
              {t("settings.devices.clips.clear")}
            </Button>
          </div>
        }
      >
        {null}
      </Dialog>
    </div>
  );
};
