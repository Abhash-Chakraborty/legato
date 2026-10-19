import React, { useEffect, useMemo, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "sonner";
import { Check, Cpu, Languages, Loader2 } from "lucide-react";
import { commands, type GpuDeviceOption, type ModelInfo } from "@/bindings";
import { useModelStore } from "../../stores/modelStore";
import { getLanguageLabel } from "../../lib/constants/languages";
import { formatModelSize } from "../../lib/utils/format";
import { isLegacySource } from "../onboarding/ModelCard";
import { Button } from "../ui/Button";
import type { StepProps } from "./SetupFlow";
import { Panel } from "./parts";
import { pickModels } from "./setup";
import { useSetupStore } from "./setupStore";

type Phase = "idle" | "download" | "loading" | "ready";

/** Two speech models to pick from, the one that suits the Windows input
 *  languages first. Continue downloads it and loads it. */
export const ModelStep: React.FC<StepProps> = ({ go, setFoot }) => {
  const { t, i18n } = useTranslation();
  const models = useModelStore((s) => s.models);
  const current = useModelStore((s) => s.currentModel);
  const initialized = useModelStore((s) => s.initialized);
  const initialize = useModelStore((s) => s.initialize);
  const downloadModel = useModelStore((s) => s.downloadModel);
  const cancelDownload = useModelStore((s) => s.cancelDownload);
  const selectModel = useModelStore((s) => s.selectModel);
  const downloading = useModelStore((s) => s.downloadingModels);
  const verifying = useModelStore((s) => s.verifyingModels);
  const progress = useModelStore((s) => s.downloadProgress);
  const openModels = useSetupStore((s) => s.openModels);
  const setOpenModels = useSetupStore((s) => s.setOpenModels);
  const [languages, setLanguages] = useState<string[] | null>(null);
  const [gpu, setGpu] = useState<GpuDeviceOption | null>(null);
  const [chosen, setChosen] = useState<string | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("idle");
  const listRef = useRef<HTMLDivElement>(null);
  const leaving = useRef<number | null>(null);

  useEffect(() => {
    if (!initialized) initialize();
    commands
      .getInputLanguages()
      .then(setLanguages)
      .catch(() => setLanguages([]));
    commands
      .getAvailableAccelerators()
      .then((a) => setGpu(a.gpu_devices[0] ?? null))
      .catch(() => {});
    return () => {
      if (leaving.current) clearTimeout(leaving.current);
    };
  }, [initialized, initialize]);

  const picks = useMemo(
    () =>
      pickModels(
        models,
        languages ?? [],
        (m) => !isLegacySource(m) || m.is_downloaded,
      ),
    [models, languages],
  );
  // A model already in use (setup run again) is offered first.
  const inUse = models.find((m) => m.id === current && m.is_downloaded);
  const cards = useMemo(() => {
    const list = [inUse, picks.first, picks.second].filter(
      (m): m is ModelInfo => !!m,
    );
    return list.filter((m, i) => list.findIndex((x) => x.id === m.id) === i);
  }, [inUse, picks]);
  const selected = chosen ?? cards[0]?.id ?? null;
  const model = cards.find((m) => m.id === selected) ?? null;

  const ready = () => {
    setPhase("ready");
    leaving.current = window.setTimeout(() => go("message"), 650);
  };

  const load = (id: string) => {
    setPhase("loading");
    selectModel(id).then((ok) => {
      if (ok) ready();
      else {
        toast.error(t("onboarding.errors.selectModel"));
        setBusy(null);
        setPhase("idle");
      }
    });
  };

  // A finished download goes straight on to loading the model.
  useEffect(() => {
    if (!busy || phase !== "download") return;
    const m = models.find((x) => x.id === busy);
    if (m?.is_downloaded && !(busy in downloading) && !(busy in verifying))
      load(busy);
  }, [busy, phase, models, downloading, verifying]);

  const start = () => {
    if (!model) return;
    const id = model.id;
    setBusy(id);
    if (model.is_downloaded) {
      if (id === current) ready();
      else load(id);
      return;
    }
    setPhase("download");
    if (id in downloading) return;
    downloadModel(id).then((ok) => {
      if (!ok) {
        setBusy(null);
        setPhase("idle");
      }
    });
  };

  const cancel = async () => {
    if (!busy) return;
    if (await cancelDownload(busy)) {
      setBusy(null);
      setPhase("idle");
      toast(t("setup.model.cancelled"), { duration: 2000 });
    }
  };

  useEffect(() => {
    const where =
      model && !model.is_downloaded
        ? t("setup.model.willDownload", {
            size: formatModelSize(Number(model.size_mb)),
          })
        : t("setup.model.onPc");
    setFoot(
      busy
        ? {
            label: t(`setup.model.phase.${phase}`),
            disabled: true,
            note:
              phase === "download" ? (
                <Button variant="ghost" size="sm" onClick={cancel}>
                  {t("setup.model.cancel")}
                </Button>
              ) : (
                ""
              ),
          }
        : {
            disabled: !model,
            note: model ? where : t("setup.model.loading"),
            onNext: start,
          },
    );
  }, [busy, phase, model, current, downloading, setFoot, t]);

  const select = (id: string) => {
    if (busy) return;
    setChosen(id);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    const keys = ["ArrowDown", "ArrowRight", "ArrowUp", "ArrowLeft"];
    if (!keys.includes(e.key) || busy || !selected) return;
    e.preventDefault();
    const i = cards.findIndex((m) => m.id === selected);
    const step = e.key === "ArrowDown" || e.key === "ArrowRight" ? 1 : -1;
    const next = cards[(i + step + cards.length) % cards.length];
    setChosen(next.id);
    listRef.current
      ?.querySelector<HTMLElement>(`[data-model="${CSS.escape(next.id)}"]`)
      ?.focus();
  };

  const names = (languages ?? [])
    .map((tag) => getLanguageLabel(tag.split("-")[0]) ?? tag)
    .filter((n, i, a) => a.indexOf(n) === i);
  const join = (list: string[]) =>
    list.length < 2
      ? list.join("")
      : t("setup.and", {
          list: list.slice(0, -1).join(", "),
          last: list[list.length - 1],
        });
  const fastest = [...cards].sort((a, b) => b.speed_score - a.speed_score)[0];
  const sharpest = [...cards].sort(
    (a, b) => b.accuracy_score - a.accuracy_score,
  )[0];

  const side = (m: ModelInfo) => {
    if (busy === m.id && phase === "download") {
      const p = progress[m.id];
      return p
        ? t("setup.model.progress", {
            percent: Math.floor(p.percentage),
            total: formatModelSize(Number(m.size_mb)),
          })
        : t("setup.model.starting");
    }
    if (busy === m.id && phase === "loading")
      return (
        <span className="flex items-center gap-1.5">
          <Loader2 className="h-3.5 w-3.5 animate-spin" aria-hidden="true" />
          {t("setup.model.phase.loading")}
        </span>
      );
    if (busy === m.id && phase === "ready")
      return (
        <span className="flex items-center gap-1 text-teal-700">
          <Check className="h-3.5 w-3.5" aria-hidden="true" />
          {t("setup.model.phase.ready")}
        </span>
      );
    if (m.is_downloaded)
      return m.id === current
        ? t("setup.model.inUse")
        : t("setup.model.downloaded");
    return formatModelSize(Number(m.size_mb));
  };

  return (
    <Panel
      title={t("setup.model.title")}
      sub={gpu ? t("setup.model.subGpu") : t("setup.model.sub")}
      left={
        <ul className="mt-6 flex flex-col gap-3 text-[13px] text-text-2">
          {gpu && (
            <li className="flex items-center gap-2.5">
              <Cpu
                className="h-4 w-4 flex-none text-text-3"
                aria-hidden="true"
              />
              {gpu.total_vram_mb > 0
                ? t("setup.model.gpu", {
                    name: gpu.name,
                    gb: Math.round(gpu.total_vram_mb / 1024),
                  })
                : gpu.name}
            </li>
          )}
          {names.length > 0 && (
            <li className="flex items-center gap-2.5">
              <Languages
                className="h-4 w-4 flex-none text-text-3"
                aria-hidden="true"
              />
              {t("setup.model.inputLanguages", { list: join(names) })}
            </li>
          )}
        </ul>
      }
      right={
        <>
          <div
            ref={listRef}
            role="radiogroup"
            aria-label={t("setup.model.title")}
            onKeyDown={onKeyDown}
            className="flex flex-col gap-3"
          >
            {cards.map((m) => {
              const on = m.id === selected;
              const mine = busy === m.id;
              const p = mine && phase === "download" ? progress[m.id] : null;
              const facts = [
                m.supported_languages.length === 1 &&
                m.supported_languages[0] === "en"
                  ? t("setup.model.english")
                  : t("setup.model.languages", {
                      count: m.supported_languages.length,
                    }),
                cards.length > 1 && m.id === fastest?.id
                  ? t("setup.model.faster")
                  : null,
                cards.length > 1 && m.id === sharpest?.id
                  ? t("setup.model.accurate")
                  : null,
              ].filter(Boolean);
              const why =
                m.id === picks.first?.id && picks.because
                  ? t("setup.model.because", {
                      language:
                        getLanguageLabel(picks.because) ?? picks.because,
                    })
                  : null;
              return (
                <div
                  key={m.id}
                  data-model={m.id}
                  role="radio"
                  aria-checked={on}
                  aria-disabled={!!busy && !mine}
                  tabIndex={on ? 0 : -1}
                  onClick={() => select(m.id)}
                  onKeyDown={(e) => {
                    if (e.key === " ") {
                      e.preventDefault();
                      select(m.id);
                    }
                  }}
                  className={`relative grid cursor-pointer grid-cols-[18px_minmax(0,1fr)_auto] gap-x-3 overflow-hidden rounded-[var(--r-lg)] border p-4 transition-colors duration-[var(--dur-1)] hover:duration-0 ${
                    on
                      ? "border-teal-600 shadow-[0_0_0_1px_var(--teal-600)]"
                      : "border-line hover:border-line-strong"
                  } ${busy && !mine ? "pointer-events-none opacity-50" : ""}`}
                >
                  <span
                    aria-hidden="true"
                    className={`mt-0.5 flex h-[18px] w-[18px] items-center justify-center rounded-full border ${
                      on ? "border-teal-600" : "border-line-strong"
                    }`}
                  >
                    {on && (
                      <span className="h-2 w-2 rounded-full bg-teal-600" />
                    )}
                  </span>
                  <div className="min-w-0">
                    <div className="text-sm font-medium">{m.name}</div>
                    <div className="mt-0.5 text-[13px] text-text-2">
                      {facts.join(" · ")}
                    </div>
                    {why && (
                      <div className="mt-1.5 text-[13px] text-teal-700">
                        {why}
                      </div>
                    )}
                  </div>
                  <div className="text-end text-[13px] tabular-nums text-text-3">
                    {side(m)}
                  </div>
                  {p && (
                    <div
                      className="absolute inset-x-0 bottom-0 h-[3px] bg-surface-2"
                      aria-hidden="true"
                    >
                      <i
                        className="block h-full bg-teal-600 transition-[width] duration-[var(--dur-4)]"
                        style={{ width: `${p.percentage}%` }}
                      />
                    </div>
                  )}
                </div>
              );
            })}
          </div>
          <div className="mt-3 flex flex-wrap items-center gap-x-3 gap-y-1 text-[13px] text-text-3">
            <button
              type="button"
              className="link"
              aria-pressed={openModels}
              onClick={() => setOpenModels(!openModels)}
            >
              {openModels ? t("setup.model.allOff") : t("setup.model.all")}
            </button>
            <span>
              {openModels
                ? t("setup.model.allNote")
                : t("setup.model.switchNote")}
            </span>
          </div>
        </>
      }
    />
  );
};
