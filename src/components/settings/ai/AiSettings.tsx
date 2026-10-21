import React, { useEffect, useId, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import {
  Check,
  Cloud,
  Copy,
  Eye,
  EyeOff,
  KeyRound,
  Monitor,
  RefreshCw,
  ShieldCheck,
  Trash2,
} from "lucide-react";
import {
  commands,
  type AiStatus,
  type PostProcessApiFormat,
  type PostProcessProvider,
} from "@/bindings";
import { useSettings } from "../../../hooks/useSettings";
import { useSettingsStore } from "../../../stores/settingsStore";
import { useAiStore, watchAi } from "../../../stores/aiStore";
import {
  useDictionaryStore,
  watchDictionary,
} from "../../../stores/dictionaryStore";
import { aiName } from "../../../lib/utils/aiStatus";
import { reveal } from "../../../lib/utils/reveal";
import { Button } from "../../ui/Button";
import { Dropdown } from "../../ui/Dropdown";
import { Input } from "../../ui/Input";
import { SettingContainer } from "../../ui/SettingContainer";
import { AiDotMark, useAiRetry } from "../../shared/AiStatusLine";

/** The providers Settings › AI offers, Ollama first because it runs on
 *  this PC. Others an earlier version set up stay listed while selected. */
const ORDER = ["ollama", "openai", "anthropic", "openrouter", "groq", "custom"];

const KEY_INFO: Record<string, { prefix: string; console: string }> = {
  openai: { prefix: "sk-", console: "platform.openai.com" },
  anthropic: { prefix: "sk-ant-", console: "console.anthropic.com" },
  openrouter: { prefix: "sk-or-", console: "openrouter.ai/keys" },
  groq: { prefix: "gsk_", console: "console.groq.com" },
};

const KEYLESS = ["ollama", "custom"];
const URL_EDITABLE = ["ollama", "custom"];
/** Seconds a dictation's AI steps may wait (as in settings.rs). */
export const AI_TIMEOUT_RANGE = [2, 120] as const;
/** Seconds a Notes AI request may wait (as in settings.rs). */
export const NOTES_AI_TIMEOUT_RANGE = [15, 600] as const;

const hostOf = (url: string) => {
  try {
    return new URL(url).host;
  } catch {
    return url.replace(/^https?:\/\//, "").split("/")[0];
  }
};

const isLocalUrl = (url: string) => {
  const host = hostOf(url)
    .replace(/:\d+$/, "")
    .replace(/^\[|\]$/g, "")
    .toLowerCase();
  return (
    host === "localhost" ||
    host.endsWith(".localhost") ||
    host === "::1" ||
    host.startsWith("127.")
  );
};

const providerIsLocal = (p: PostProcessProvider) =>
  p.id === "ollama" || isLocalUrl(p.base_url);

const WhereTag: React.FC<{ local: boolean }> = ({ local }) => {
  const { t } = useTranslation();
  const Icon = local ? Monitor : Cloud;
  return (
    <span className="inline-flex flex-none items-center gap-1 text-xs text-text-3">
      <Icon className="h-3.5 w-3.5 stroke-[1.75]" aria-hidden="true" />
      {t(local ? "settings.ai.where.local" : "settings.ai.where.cloud")}
    </span>
  );
};

const useProviders = () => {
  const { settings } = useSettings();
  const all = settings?.post_process_providers ?? [];
  const selectedId = settings?.post_process_provider_id ?? "ollama";
  const listed = ORDER.map((id) => all.find((p) => p.id === id)).filter(
    (p): p is PostProcessProvider => !!p,
  );
  const selected = all.find((p) => p.id === selectedId);
  if (selected && !ORDER.includes(selected.id)) listed.push(selected);
  return { listed, selected, selectedId, settings };
};

/** Focuses a field in the provider panel once it is on screen. */
const focusLater = (id: string) =>
  requestAnimationFrame(() => {
    const el = document.getElementById(id);
    if (el) {
      reveal(el);
      el.focus({ preventScroll: true });
    }
  });

/* ------------------------------------------------------------ status card */

/** The connection at the top of Settings › AI: what is set up, how it last
 *  answered, and the one thing to do about a problem. */
export const AiStatusCard: React.FC = () => {
  const { t } = useTranslation();
  const status = useAiStore((s) => s.status);
  const testing = useAiStore((s) => s.testing);
  const keyHints = useAiStore((s) => s.keyHints);
  const retry = useAiRetry();
  const { setPostProcessProvider, settings } = useSettings();
  useEffect(watchAi, []);

  if (!status) return null;
  const name = aiName(status, t);
  const seconds = settings?.ai_timeout_secs ?? 6;
  const v = { name, model: status.model };
  const host = hostOf(status.base_url);
  const console = KEY_INFO[status.provider_id]?.console;
  const last4 = keyHints[status.provider_id] ?? "";

  const retryButton = (
    <Button variant="secondary" size="sm" onClick={() => void retry()}>
      {t("settings.ai.actions.retry")}
    </Button>
  );
  const useOllama = (primary = false) => (
    <Button
      variant={primary ? "primary" : "secondary"}
      size="sm"
      onClick={() => void setPostProcessProvider("ollama")}
    >
      {t("settings.ai.actions.useOllama")}
    </Button>
  );

  let dot: "ok" | "off" | "danger" = "danger";
  let title = "";
  let help: React.ReactNode = "";
  let actions: React.ReactNode = retryButton;

  if (testing) {
    dot = "off";
    title = t("settings.ai.status.testingTitle", v);
    help = t("settings.ai.status.testingHelp");
    actions = (
      <Button variant="secondary" size="sm" disabled>
        {t("settings.ai.actions.test")}
      </Button>
    );
  } else if (status.kind === "none") {
    dot = "off";
    if (status.needs_key && !status.has_key) {
      title = t("settings.ai.status.noKeyTitle");
      help = t("settings.ai.status.noKeyHelp", v);
      actions = useOllama();
    } else {
      title =
        status.provider_id === "custom"
          ? t("settings.ai.status.noModelServerTitle")
          : t("settings.ai.status.noModelTitle", v);
      help = t("settings.ai.status.noModelHelp");
      actions = null;
    }
  } else if (status.kind === "unreachable") {
    if (status.provider_id === "ollama") {
      title = t("settings.ai.status.unreachableOllamaTitle");
      help = t("settings.ai.status.unreachableOllamaHelp", { host });
    } else if (status.provider_id === "custom") {
      title = t("settings.ai.status.unreachableServerTitle");
      help = t("settings.ai.status.unreachableServerHelp", {
        url: status.base_url,
      });
    } else {
      title = t("settings.ai.status.unreachableTitle", v);
      help = t("settings.ai.status.unreachableHelp", { host });
    }
  } else if (status.kind === "rejected") {
    if (status.has_key) {
      title = t("settings.ai.status.rejectedKeyTitle", v);
      help = console
        ? t("settings.ai.status.rejectedKeyHelp", { last4, console })
        : t("settings.ai.status.rejectedKeyHelpPlain", { last4 });
      actions = (
        <Button
          variant="primary"
          size="sm"
          onClick={() => {
            window.dispatchEvent(new CustomEvent("legato:replace-key"));
            focusLater("ai-key");
          }}
        >
          {t("settings.ai.actions.replaceKey")}
        </Button>
      );
    } else {
      title = t("settings.ai.status.rejectedTitle", v);
      help = t("settings.ai.status.rejectedHelp");
    }
  } else if (status.kind === "not_found") {
    title = t("settings.ai.status.notFoundTitle", v);
    if (status.provider_id === "ollama") {
      const command = `ollama pull ${status.model}`;
      help = t("settings.ai.status.notFoundOllamaHelp", { command });
      actions = (
        <>
          <Button
            variant="secondary"
            size="sm"
            onClick={() =>
              navigator.clipboard
                .writeText(command)
                .then(() => toast.success(t("settings.ai.copied", { command })))
                .catch(() => {})
            }
          >
            <Copy className="h-3.5 w-3.5 stroke-[1.75]" aria-hidden="true" />
            {t("settings.ai.actions.copyCommand")}
          </Button>
          {retryButton}
        </>
      );
    } else {
      help = t("settings.ai.status.notFoundHelp");
      actions = (
        <Button
          variant="primary"
          size="sm"
          onClick={() => focusLater("ai-model")}
        >
          {t("settings.ai.actions.chooseModel")}
        </Button>
      );
    }
  } else if (status.kind === "rate_limited") {
    title = t(
      status.local
        ? "settings.ai.status.rateLimitedLocalTitle"
        : "settings.ai.status.rateLimitedTitle",
      v,
    );
    help = t(
      status.local
        ? "settings.ai.status.rateLimitedLocalHelp"
        : "settings.ai.status.rateLimitedHelp",
    );
  } else if (status.kind === "slow") {
    title = t("settings.ai.status.slowTitle", v);
    help = t("settings.ai.status.slowHelp", { seconds });
  } else if (status.kind === "failed") {
    title = t("settings.ai.status.failedTitle", v);
    help = t("settings.ai.status.failedHelp");
  } else {
    dot = status.kind === "ok" ? "ok" : "off";
    title = t("settings.ai.status.connectedTitle", v);
    help =
      status.kind === "untested"
        ? t("settings.ai.status.untestedHelp")
        : status.tested_at
          ? t("settings.ai.status.okHelp", {
              time: new Date(status.tested_at * 1000).toLocaleString([], {
                hour: "numeric",
                minute: "2-digit",
                ...(Date.now() / 1000 - status.tested_at > 20 * 3600
                  ? { day: "numeric", month: "short" }
                  : {}),
              }),
              ms: status.latency_ms ?? 0,
            })
          : t("settings.ai.status.okHelpPlain");
    actions = (
      <Button variant="secondary" size="sm" onClick={() => void retry()}>
        {t("settings.ai.actions.test")}
      </Button>
    );
  }

  return (
    <div
      aria-live="polite"
      className="flex flex-wrap items-center gap-x-6 gap-y-3 rounded-[var(--r-md)] border border-line bg-surface px-4 py-3.5"
    >
      <div className="flex min-w-0 flex-[1_1_280px] items-start gap-2.5">
        <span className="mt-[7px]">
          <AiDotMark dot={dot} />
        </span>
        <div className="min-w-0">
          <div className="flex flex-wrap items-center gap-x-2.5">
            <h3 className="text-[15px] font-medium leading-snug">{title}</h3>
            {(status.kind === "ok" || status.kind === "untested") &&
              !testing && <WhereTag local={status.local} />}
          </div>
          <p className="mt-0.5 text-[13px] leading-[1.45] text-text-3">
            {help}
          </p>
        </div>
      </div>
      {actions && <div className="flex flex-none gap-2">{actions}</div>}
    </div>
  );
};

/* ----------------------------------------------------------- model picker */

/** Pick a model from the provider's list or type its exact name. */
const ModelCombo: React.FC<{
  value: string;
  options: string[];
  onCommit: (model: string) => void;
  label: string;
}> = ({ value, options, onCommit, label }) => {
  const { t } = useTranslation();
  const [text, setText] = useState(value);
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(-1);
  const listId = useId();
  const listRef = useRef<HTMLUListElement>(null);

  useEffect(() => setText(value), [value]);

  const typed = text.trim();
  const matches =
    typed && typed !== value
      ? options.filter((m) => m.toLowerCase().includes(typed.toLowerCase()))
      : options;

  const commit = (model: string) => {
    setOpen(false);
    setActive(-1);
    setText(model);
    if (model.trim() !== value) onCommit(model.trim());
  };

  useEffect(() => {
    if (open && listRef.current) reveal(listRef.current);
  }, [open]);

  return (
    <div className="relative">
      <input
        id="ai-model"
        className="input w-[260px]"
        role="combobox"
        aria-label={label}
        aria-expanded={open}
        aria-controls={listId}
        aria-autocomplete="list"
        aria-activedescendant={
          open && active >= 0 ? `${listId}-${active}` : undefined
        }
        autoComplete="off"
        spellCheck={false}
        placeholder={t("settings.ai.model.placeholder")}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
          setOpen(true);
          setActive(-1);
        }}
        onFocus={() => options.length > 0 && setOpen(true)}
        onBlur={() => {
          // A click on an option lands first; a blur elsewhere keeps typing.
          setTimeout(() => {
            setOpen(false);
            if (text.trim() && text.trim() !== value) commit(text);
          }, 120);
        }}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setOpen(true);
            setActive((i) => Math.min(i + 1, matches.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setActive((i) => Math.max(i - 1, 0));
          } else if (e.key === "Enter") {
            e.preventDefault();
            commit(open && active >= 0 ? matches[active] : text);
          } else if (e.key === "Escape" && open) {
            e.preventDefault();
            e.stopPropagation();
            setOpen(false);
            setText(value);
          }
        }}
      />
      {open && matches.length > 0 && (
        <ul
          ref={listRef}
          id={listId}
          role="listbox"
          aria-label={label}
          className="menu absolute end-0 top-full z-30 mt-1 max-h-64 w-full scroll-my-4 overflow-y-auto"
        >
          {matches.map((m, i) => (
            <li
              key={m}
              id={`${listId}-${i}`}
              role="option"
              aria-selected={m === value}
              className={`menu-item ${i === active ? "is-active" : ""}`}
              onMouseDown={(e) => {
                e.preventDefault();
                commit(m);
              }}
            >
              <Check
                className={`h-4 w-4 flex-none stroke-[2] text-teal-700 ${m === value ? "" : "invisible"}`}
                aria-hidden="true"
              />
              <span className="min-w-0 flex-1 truncate">{m}</span>
            </li>
          ))}
        </ul>
      )}
    </div>
  );
};

/* -------------------------------------------------------- provider panel */

const KeyField: React.FC<{ provider: PostProcessProvider }> = ({
  provider,
}) => {
  const { t } = useTranslation();
  const hint = useAiStore((s) => s.keyHints[provider.id]);
  const load = useAiStore((s) => s.load);
  const [editing, setEditing] = useState(!hint);
  const [shown, setShown] = useState(false);
  const [value, setValue] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const info = KEY_INFO[provider.id];
  const optional = KEYLESS.includes(provider.id);
  const name = provider.id === "custom" ? t("ai.yourServer") : provider.label;

  useEffect(() => setEditing(!hint), [hint, provider.id]);
  useEffect(() => {
    const onReplace = () => setEditing(true);
    window.addEventListener("legato:replace-key", onReplace);
    return () => window.removeEventListener("legato:replace-key", onReplace);
  }, []);

  const save = async (key: string) => {
    setBusy(true);
    setError(null);
    const result = await commands
      .changePostProcessApiKeySetting(provider.id, key)
      .catch((e) => ({ status: "error" as const, error: String(e) }));
    setBusy(false);
    if (result.status === "error") {
      setError(t("settings.ai.key.failed", { error: result.error }));
      return false;
    }
    await load();
    useSettingsStore.setState({ savedAt: Date.now() });
    return true;
  };

  const help = error ? (
    <span className="text-danger">{error}</span>
  ) : optional ? (
    t("settings.ai.key.optionalHelp")
  ) : (
    t("settings.ai.key.help", {
      prefix: info?.prefix ?? "",
      host: hostOf(provider.base_url),
    })
  );

  return (
    <SettingContainer
      title={t(
        optional ? "settings.ai.key.optionalTitle" : "settings.ai.key.title",
      )}
      description={help}
    >
      <div className="flex items-center">
        {hint && !editing ? (
          <div className="flex items-center gap-2">
            <span className="font-mono text-[13px] text-text-2">
              {(info?.prefix ?? "") + "••••••••" + hint}
            </span>
            <Button
              variant="secondary"
              size="sm"
              onClick={() => {
                setEditing(true);
                focusLater("ai-key");
              }}
            >
              {t("settings.ai.key.replace")}
            </Button>
            <button
              type="button"
              className="grid h-8 w-8 place-items-center rounded-[var(--r-sm)] text-text-2 transition-colors duration-[var(--dur-1)] hover:bg-surface-3 hover:text-danger hover:duration-0"
              aria-label={t("settings.ai.key.remove", { name })}
              title={t("settings.ai.key.removeTip")}
              onClick={async () => {
                if (await save("")) toast.success(t("settings.ai.key.removed"));
              }}
            >
              <Trash2 className="h-4 w-4 stroke-[1.75]" aria-hidden="true" />
            </button>
          </div>
        ) : (
          <form
            className="flex items-center gap-2"
            onSubmit={async (e) => {
              e.preventDefault();
              if (!value.trim()) return;
              if (await save(value)) {
                setValue("");
                setShown(false);
                setEditing(false);
                toast.success(t("settings.ai.key.saved"));
                void useSettingsStore
                  .getState()
                  .fetchPostProcessModels(provider.id);
              }
            }}
          >
            <span className="relative">
              <input
                id="ai-key"
                className={`input w-[240px] pe-9 font-mono ${error ? "invalid" : ""}`}
                type={shown ? "text" : "password"}
                autoComplete="off"
                spellCheck={false}
                aria-label={t("settings.ai.key.label", { name })}
                placeholder={info ? `${info.prefix}…` : ""}
                value={value}
                onChange={(e) => {
                  setValue(e.target.value);
                  setError(null);
                }}
                onKeyDown={(e) => {
                  if (e.key === "Escape" && hint) {
                    e.stopPropagation();
                    setEditing(false);
                    setValue("");
                  }
                }}
              />
              <button
                type="button"
                className="absolute end-1 top-1/2 grid h-7 w-7 -translate-y-1/2 place-items-center rounded-[var(--r-xs)] text-text-3 hover:text-text"
                aria-label={t(
                  shown ? "settings.ai.key.hide" : "settings.ai.key.show",
                )}
                aria-pressed={shown}
                onClick={() => setShown(!shown)}
              >
                {shown ? (
                  <EyeOff
                    className="h-4 w-4 stroke-[1.75]"
                    aria-hidden="true"
                  />
                ) : (
                  <Eye className="h-4 w-4 stroke-[1.75]" aria-hidden="true" />
                )}
              </button>
            </span>
            <Button
              type="submit"
              variant="primary"
              size="sm"
              disabled={busy || !value.trim()}
            >
              {t("settings.ai.key.save")}
            </Button>
            {hint && (
              <Button
                variant="ghost"
                size="sm"
                onClick={() => {
                  setEditing(false);
                  setValue("");
                  setError(null);
                }}
              >
                {t("settings.ai.key.cancel")}
              </Button>
            )}
          </form>
        )}
      </div>
    </SettingContainer>
  );
};

const BaseUrlField: React.FC<{ provider: PostProcessProvider }> = ({
  provider,
}) => {
  const { t } = useTranslation();
  const { updatePostProcessBaseUrl } = useSettings();
  const [value, setValue] = useState(provider.base_url);
  const [error, setError] = useState(false);
  useEffect(() => setValue(provider.base_url), [provider.base_url]);

  const commit = () => {
    const url = value.trim();
    if (url === provider.base_url) return;
    if (!/^https?:\/\/\S+$/i.test(url)) {
      setError(true);
      return;
    }
    setError(false);
    void updatePostProcessBaseUrl(provider.id, url);
  };

  return (
    <SettingContainer
      title={t("settings.ai.baseUrl.title")}
      description={
        error
          ? t("settings.ai.baseUrl.invalid")
          : t(
              provider.id === "ollama"
                ? "settings.ai.baseUrl.ollamaHelp"
                : "settings.ai.baseUrl.customHelp",
            )
      }
    >
      <input
        className={`input w-[260px] font-mono text-[13px] ${error ? "invalid" : ""}`}
        aria-label={t("settings.ai.baseUrl.label", {
          name: provider.id === "custom" ? t("ai.yourServer") : provider.label,
        })}
        aria-invalid={error || undefined}
        spellCheck={false}
        autoComplete="off"
        value={value}
        onChange={(e) => {
          setValue(e.target.value);
          setError(false);
        }}
        onBlur={commit}
        onKeyDown={(e) => {
          if (e.key === "Enter") commit();
          if (e.key === "Escape") {
            e.stopPropagation();
            setValue(provider.base_url);
            setError(false);
          }
        }}
      />
    </SettingContainer>
  );
};

const ModelField: React.FC<{ provider: PostProcessProvider }> = ({
  provider,
}) => {
  const { t } = useTranslation();
  const { settings, postProcessModelOptions, refreshSettings } = useSettings();
  const hasKey = useAiStore((s) => !!s.keyHints[provider.id]);
  const retry = useAiRetry();
  const [fetching, setFetching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const model = settings?.post_process_models?.[provider.id] ?? "";
  const options = postProcessModelOptions[provider.id] ?? [];
  const canFetch = KEYLESS.includes(provider.id) || hasKey;

  const fetchModels = async (quiet = false) => {
    setFetching(true);
    setError(null);
    const result = await commands
      .fetchPostProcessModels(provider.id)
      .catch((e) => ({ status: "error" as const, error: String(e) }));
    setFetching(false);
    if (result.status === "ok") {
      useSettingsStore
        .getState()
        .setPostProcessModelOptions(provider.id, result.data);
      if (!quiet && result.data.length === 0)
        setError(t("settings.ai.model.none"));
    } else if (!quiet) {
      // The first part says what failed; the rest is the server's own text.
      const reason = result.error.split(": ")[0].slice(0, 140);
      setError(t("settings.ai.model.fetchFailed", { reason }));
    }
  };

  // A provider that can answer gets its list as soon as it is chosen.
  useEffect(() => {
    if (canFetch && options.length === 0) void fetchModels(true);
  }, [provider.id, provider.base_url, canFetch]);

  return (
    <SettingContainer
      title={t("settings.ai.model.title")}
      description={error ?? t("settings.ai.model.help")}
    >
      <ModelCombo
        value={model}
        options={options}
        label={t("settings.ai.model.label")}
        onCommit={async (next) => {
          const result = await commands.changePostProcessModelSetting(
            provider.id,
            next,
          );
          if (result.status === "error") return;
          await refreshSettings();
          useSettingsStore.setState({ savedAt: Date.now() });
          if (next) void retry();
        }}
      />
      <Button
        variant="secondary"
        size="sm"
        disabled={!canFetch || fetching}
        onClick={() => void fetchModels()}
      >
        <RefreshCw
          className={`h-3.5 w-3.5 stroke-[1.75] ${fetching ? "animate-spin motion-reduce:animate-none" : ""}`}
          aria-hidden="true"
        />
        {t(fetching ? "settings.ai.model.fetching" : "settings.ai.model.fetch")}
      </Button>
    </SettingContainer>
  );
};

const ApiFormatField: React.FC<{ provider: PostProcessProvider }> = ({
  provider,
}) => {
  const { t } = useTranslation();
  const { updatePostProcessApiFormat } = useSettings();
  return (
    <SettingContainer
      title={t("settings.ai.apiFormat.title")}
      description={t("settings.ai.apiFormat.help")}
    >
      <Dropdown
        className="w-[220px]"
        ariaLabel={t("settings.ai.apiFormat.title")}
        selectedValue={provider.api_format ?? "openai"}
        options={(["openai", "anthropic"] as PostProcessApiFormat[]).map(
          (f) => ({ value: f, label: t(`settings.ai.apiFormat.${f}`) }),
        )}
        onSelect={(f) =>
          void updatePostProcessApiFormat(
            provider.id,
            f as PostProcessApiFormat,
          )
        }
      />
    </SettingContainer>
  );
};

/* --------------------------------------------------------- provider list */

/** The providers as a radio list; the chosen one opens its address, key
 *  and model underneath. */
export const AiProviders: React.FC = () => {
  const { t } = useTranslation();
  const { listed, selectedId } = useProviders();
  const { setPostProcessProvider } = useSettings();
  const keyHints = useAiStore((s) => s.keyHints);
  const refs = useRef<Record<string, HTMLDivElement | null>>({});
  useEffect(watchAi, []);

  const choose = (id: string) => {
    if (id !== selectedId) void setPostProcessProvider(id);
  };

  const onKeyDown = (e: React.KeyboardEvent, index: number) => {
    const move = (to: number) => {
      e.preventDefault();
      const next = listed[(to + listed.length) % listed.length];
      choose(next.id);
      refs.current[next.id]?.focus();
    };
    if (e.key === "ArrowDown" || e.key === "ArrowRight") move(index + 1);
    else if (e.key === "ArrowUp" || e.key === "ArrowLeft") move(index - 1);
    else if (e.key === " " || e.key === "Enter") {
      e.preventDefault();
      choose(listed[index].id);
    }
  };

  return (
    <div className="py-3.5">
      <h3 data-setting-title className="text-[15px] font-medium leading-snug">
        {t("settings.ai.provider.title")}
      </h3>
      <p
        data-setting-help
        className="mt-0.5 text-[13px] leading-[1.45] text-text-3"
      >
        {t("settings.ai.provider.help")}
      </p>
      <div
        role="radiogroup"
        aria-label={t("settings.ai.provider.label")}
        className="mt-3 divide-y divide-line overflow-hidden rounded-[var(--r-md)] border border-line"
      >
        {listed.map((p, i) => {
          const on = p.id === selectedId;
          const known = ORDER.includes(p.id);
          return (
            <div key={p.id}>
              <div
                ref={(el) => {
                  refs.current[p.id] = el;
                }}
                role="radio"
                aria-checked={on}
                tabIndex={on ? 0 : -1}
                onClick={() => choose(p.id)}
                onKeyDown={(e) => onKeyDown(e, i)}
                className="flex cursor-pointer items-center gap-3 px-4 py-3 transition-colors duration-[var(--dur-1)] hover:bg-surface-2 hover:duration-0 ring-inside"
              >
                <span
                  aria-hidden="true"
                  className={`grid h-4 w-4 flex-none place-items-center rounded-full border transition-colors duration-[var(--dur-2)] ${on ? "border-teal-700" : "border-line-strong"}`}
                >
                  <span
                    className={`h-2 w-2 rounded-full bg-teal-700 transition-transform duration-[var(--dur-2)] ease-[var(--ease-spring)] ${on ? "scale-100" : "scale-0"}`}
                  />
                </span>
                <span className="min-w-0 flex-1">
                  <span className="block text-sm font-medium">{p.label}</span>
                  <span className="block text-[13px] text-text-3">
                    {t(
                      known
                        ? `settings.ai.providers.${p.id}`
                        : "settings.ai.providers.legacy",
                    )}
                  </span>
                </span>
                {!KEYLESS.includes(p.id) && keyHints[p.id] && (
                  <span className="inline-flex flex-none items-center gap-1 text-xs text-text-3">
                    <KeyRound
                      className="h-3.5 w-3.5 stroke-[1.75]"
                      aria-hidden="true"
                    />
                    {t("settings.ai.keySaved")}
                  </span>
                )}
                <WhereTag local={providerIsLocal(p)} />
              </div>
              {on && (
                <div
                  role="group"
                  aria-label={t("settings.ai.provider.panel", {
                    name: p.label,
                  })}
                  className="divide-y divide-line bg-surface-2 px-4 ps-11"
                >
                  {URL_EDITABLE.includes(p.id) && <BaseUrlField provider={p} />}
                  {p.id === "custom" && <ApiFormatField provider={p} />}
                  {(!KEYLESS.includes(p.id) || p.id === "custom") && (
                    <KeyField provider={p} />
                  )}
                  <ModelField provider={p} />
                </div>
              )}
            </div>
          );
        })}
      </div>
    </div>
  );
};

/* ------------------------------------------------------------- uses */

const WhereOfAi: React.FC = () => {
  const status = useAiStore((s) => s.status);
  return status && status.kind !== "none" ? (
    <WhereTag local={status.local} />
  ) : null;
};

const Toggle: React.FC<{
  checked: boolean;
  label: string;
  onChange: (on: boolean) => void;
  busy?: boolean;
}> = ({ checked, label, onChange, busy }) => (
  <button
    type="button"
    role="switch"
    aria-checked={checked}
    aria-label={label}
    aria-busy={busy || undefined}
    className="toggle"
    onClick={() => !busy && onChange(!checked)}
  />
);

export const CorrectionPassRow: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const count = useDictionaryStore((s) => s.view?.entries.length ?? 0);
  useEffect(watchDictionary, []);
  const label = t("settings.ai.use.correction.title");
  return (
    <SettingContainer
      title={label}
      description={
        count
          ? t("settings.ai.use.correction.help", { count })
          : t("settings.ai.use.correction.helpEmpty")
      }
    >
      <WhereOfAi />
      <Toggle
        label={label}
        checked={getSetting("ai_correction_enabled") ?? false}
        busy={isUpdating("ai_correction_enabled")}
        onChange={(on) => updateSetting("ai_correction_enabled", on)}
      />
    </SettingContainer>
  );
};

export const TransformsUseRow: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const label = t("settings.ai.use.transforms.title");
  return (
    <SettingContainer
      title={label}
      description={t("settings.ai.use.transforms.help")}
    >
      <WhereOfAi />
      <Toggle
        label={label}
        checked={getSetting("post_process_enabled") ?? false}
        busy={isUpdating("post_process_enabled")}
        onChange={(on) => updateSetting("post_process_enabled", on)}
      />
    </SettingContainer>
  );
};

export const NotesSummaryRow: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting, isUpdating } = useSettings();
  const label = t("settings.ai.use.notes.title");
  return (
    <SettingContainer
      title={label}
      description={t("settings.ai.use.notes.help")}
    >
      <WhereOfAi />
      <Toggle
        label={label}
        checked={getSetting("ai_notes_enabled") ?? true}
        busy={isUpdating("ai_notes_enabled")}
        onChange={(on) => updateSetting("ai_notes_enabled", on)}
      />
    </SettingContainer>
  );
};

export const MinWordsRow: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();
  const words = getSetting("ai_min_words") ?? 8;
  const label = t("settings.ai.limits.minWords.title");
  return (
    <SettingContainer
      title={label}
      description={
        words
          ? t("settings.ai.limits.minWords.help", { count: words })
          : t("settings.ai.limits.minWords.helpOff")
      }
    >
      <input
        type="range"
        className="slider w-40"
        aria-label={label}
        aria-valuetext={
          words
            ? t("settings.ai.limits.minWords.value", { count: words })
            : t("settings.ai.limits.minWords.valueOff")
        }
        min={0}
        max={30}
        step={1}
        value={words}
        onChange={(e) => updateSetting("ai_min_words", Number(e.target.value))}
      />
      <span className="w-[72px] text-end text-[13px] tabular-nums text-text-2">
        {words
          ? t("settings.ai.limits.minWords.value", { count: words })
          : t("settings.ai.limits.minWords.valueOff")}
      </span>
    </SettingContainer>
  );
};

/** A number of seconds typed in, kept within `min`–`max` and saved when
 *  the field loses focus or on Enter. */
const SecondsField: React.FC<{
  value: number;
  min: number;
  max: number;
  label: string;
  onSave: (seconds: number) => void;
}> = ({ value, min, max, label, onSave }) => {
  const { t } = useTranslation();
  const [draft, setDraft] = useState(String(value));
  useEffect(() => setDraft(String(value)), [value]);
  const save = () => {
    const typed = Math.round(Number(draft));
    if (!Number.isFinite(typed) || draft.trim() === "") {
      setDraft(String(value));
      return;
    }
    const seconds = Math.min(max, Math.max(min, typed));
    setDraft(String(seconds));
    if (seconds !== value) onSave(seconds);
  };
  return (
    <label className="flex items-center gap-2 text-[13px] text-text-2">
      <Input
        type="number"
        variant="compact"
        inputMode="numeric"
        className="w-20 text-end tabular-nums"
        aria-label={label}
        min={min}
        max={max}
        step={1}
        value={draft}
        onChange={(e) => setDraft(e.target.value)}
        onBlur={save}
        onKeyDown={(e) => {
          if (e.key === "Enter") e.currentTarget.blur();
        }}
      />
      {t("settings.ai.limits.timeout.unit")}
    </label>
  );
};

export const TimeoutRow: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();
  const seconds = getSetting("ai_timeout_secs") ?? 6;
  return (
    <SettingContainer
      title={t("settings.ai.limits.timeout.title")}
      description={t("settings.ai.limits.timeout.help", {
        min: AI_TIMEOUT_RANGE[0],
        max: AI_TIMEOUT_RANGE[1],
      })}
    >
      <SecondsField
        value={seconds}
        min={AI_TIMEOUT_RANGE[0]}
        max={AI_TIMEOUT_RANGE[1]}
        label={t("settings.ai.limits.timeout.title")}
        onSave={(v) => updateSetting("ai_timeout_secs", v)}
      />
    </SettingContainer>
  );
};

export const NotesTimeoutRow: React.FC = () => {
  const { t } = useTranslation();
  const { getSetting, updateSetting } = useSettings();
  const seconds = getSetting("notes_ai_timeout_secs") ?? 120;
  return (
    <SettingContainer
      title={t("settings.ai.limits.notesTimeout.title")}
      description={t("settings.ai.limits.notesTimeout.help", {
        min: NOTES_AI_TIMEOUT_RANGE[0],
        max: NOTES_AI_TIMEOUT_RANGE[1],
      })}
    >
      <SecondsField
        value={seconds}
        min={NOTES_AI_TIMEOUT_RANGE[0]}
        max={NOTES_AI_TIMEOUT_RANGE[1]}
        label={t("settings.ai.limits.notesTimeout.title")}
        onSave={(v) => updateSetting("notes_ai_timeout_secs", v)}
      />
    </SettingContainer>
  );
};

/** Where text for the AI goes, in one sentence. */
export const AiPrivacyNote: React.FC = () => {
  const { t } = useTranslation();
  const status: AiStatus | null = useAiStore((s) => s.status);
  if (!status || status.kind === "none") return null;
  const name = aiName(status, t);
  return (
    <p className="flex items-start gap-2.5 py-3.5 text-[13px] leading-[1.45] text-text-3">
      <ShieldCheck
        className="mt-px h-4 w-4 flex-none stroke-[1.75] text-text-2"
        aria-hidden="true"
      />
      {status.local
        ? t("settings.ai.privacy.local", { name })
        : t("settings.ai.privacy.cloud", {
            name,
            host: hostOf(status.base_url),
          })}
    </p>
  );
};
