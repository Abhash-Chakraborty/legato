import React, { useCallback, useEffect, useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { ArrowRight, Check } from "lucide-react";
import { commands, events, type DictionaryEntry } from "@/bindings";
import { Button } from "../ui/Button";
import type { StepProps } from "./SetupFlow";
import { AppGlyph, Panel } from "./parts";
import { SceneText, type Mark } from "./SceneApps";
import { PRACTICE, SCENE_TERMS, termAsHeard, type Scene } from "./setup";
import { useSetupStore } from "./setupStore";

interface Term {
  key: string;
  term: string;
  scene: Scene;
  /** How the practice wrote it, when the scene was dictated. */
  heard: string | null;
}

type RowState = "pending" | "added" | "skipped" | "known";

/** Words from the practice the Dictionary doesn't know, to add or skip,
 *  with the email as it would come out next time. */
export const WordsStep: React.FC<StepProps> = ({ go, setFoot }) => {
  const { t, i18n } = useTranslation();
  const results = useSetupStore((s) => s.results);
  const words = useSetupStore((s) => s.words);
  const setWord = useSetupStore((s) => s.setWord);
  const [entries, setEntries] = useState<DictionaryEntry[] | null>(null);
  const [drafts, setDrafts] = useState<Record<string, string>>({});
  const [invalid, setInvalid] = useState<string | null>(null);
  const [preview, setPreview] = useState<string | null>(null);

  const terms: Term[] = useMemo(
    () =>
      PRACTICE.flatMap((scene) =>
        SCENE_TERMS[scene].map((term) => ({
          key: term.toLowerCase(),
          term,
          scene,
          heard: results[scene]
            ? termAsHeard(
                term,
                t(`setup.scenes.${scene}.script`),
                results[scene]!.text,
              )
            : null,
        })),
      ),
    [results, t],
  );

  const loadEntries = useCallback(() => {
    commands
      .getDictionary()
      .then((r) => r.status === "ok" && setEntries(r.data.entries))
      .catch(() => setEntries([]));
  }, []);
  useEffect(() => {
    loadEntries();
    const off = events.dictionaryChanged.listen(loadEntries);
    return () => {
      off.then((fn) => fn());
    };
  }, [loadEntries]);

  const find = (text: string) =>
    entries?.find((e) => e.text.toLowerCase() === text.trim().toLowerCase());
  const stateOf = (term: Term): RowState =>
    words[term.key]?.state ?? (find(term.term) ? "known" : "pending");

  // The email as it would come out now, with the Dictionary as it is.
  const email = results.email;
  useEffect(() => {
    if (!email) return;
    commands
      .previewStyle(email.heard, "email", email.tone, null)
      .then(setPreview)
      .catch(() => {});
  }, [email, entries]);

  const counts = { pending: 0, added: 0, skipped: 0, known: 0 };
  terms.forEach((term) => counts[stateOf(term)]++);
  const pendings = terms.filter((term) => stateOf(term) === "pending");
  // A word the practice got wrong explains the step best.
  const misheard = pendings.find(
    (term) => term.heard && term.heard.toLowerCase() !== term.key,
  );

  useEffect(() => {
    setFoot(
      counts.pending && !counts.added
        ? {
            label: t("setup.words.skipForNow"),
            kind: "secondary",
            onNext: () => go("done"),
          }
        : {
            note: counts.added
              ? t("setup.words.added", { count: counts.added })
              : "",
            onNext: () => go("done"),
          },
    );
  }, [counts.pending, counts.added, go, setFoot, t]);

  const add = async (term: Term) => {
    const value = (drafts[term.key] ?? term.term).trim().replace(/\s+/g, " ");
    if (!value) {
      setInvalid(term.key);
      return;
    }
    const existing = find(value);
    if (existing) {
      setWord(term.key, { state: "added", text: existing.text });
      return;
    }
    const heard =
      term.heard && term.heard.toLowerCase() !== value.toLowerCase()
        ? [term.heard]
        : [];
    const r = await commands.addDictionaryEntry({ text: value, heard });
    if (r.status === "ok")
      setWord(term.key, {
        state: "added",
        text: r.data.text,
        entryId: r.data.id,
      });
  };

  const undo = async (term: Term) => {
    const w = words[term.key];
    if (w?.entryId) await commands.deleteDictionaryEntries([w.entryId]);
    setWord(term.key, null);
  };

  const quote = (s: string) => t("setup.quote", { text: s });
  const sub = misheard
    ? t("setup.words.subHeard", { heard: misheard.heard, word: misheard.term })
    : pendings.length
      ? t("setup.words.subPending")
      : counts.added
        ? t("setup.words.subDone")
        : t("setup.words.subKnown");

  // The email's body, between the greeting and the sign-off.
  const body = (() => {
    if (!preview) return null;
    const paras = preview.split(/\n{2,}/);
    return paras.length >= 3 ? paras.slice(1, -1).join("\n\n") : preview;
  })();
  const marks: Mark[] = terms
    .filter((term) => term.scene === "email")
    .flatMap((term): Mark[] => {
      if (body?.includes(term.term))
        return [
          {
            text: term.term,
            kind: "fixed",
            tip: t("setup.words.fixedTip", { word: term.term }),
          },
        ];
      return term.heard && term.heard !== term.term
        ? [
            {
              text: term.heard,
              kind: "unknown",
              tip: t("setup.did.unknownTip", { word: term.term }),
            },
          ]
        : [];
    });

  return (
    <Panel
      title={t("setup.words.title")}
      sub={sub}
      left={
        <p className="mt-4 text-[13px] leading-relaxed text-text-3">
          {t("setup.words.also")}
        </p>
      }
      right={
        <>
          <div
            role="group"
            aria-label={t("setup.words.group")}
            className="overflow-hidden rounded-[var(--r-lg)] border border-line"
          >
            {terms.map((term) => {
              const state = stateOf(term);
              const w = words[term.key];
              const entry = find(w?.text ?? term.term);
              const same = !term.heard || term.heard.toLowerCase() === term.key;
              const value = drafts[term.key] ?? term.term;
              return (
                <div
                  key={term.key}
                  className="grid min-h-14 grid-cols-[minmax(0,96px)_14px_minmax(0,140px)_minmax(0,1fr)_auto] items-center gap-3 border-b border-line px-4 py-2.5 last:border-b-0"
                >
                  <span
                    className="truncate text-[13px] text-text-3"
                    title={term.heard ?? term.term}
                  >
                    {term.heard ? (same ? term.heard : quote(term.heard)) : "—"}
                  </span>
                  <ArrowRight
                    className="h-3.5 w-3.5 text-text-4"
                    aria-hidden="true"
                  />
                  {state === "pending" ? (
                    <input
                      className={`input compact w-full ${invalid === term.key ? "invalid" : ""}`}
                      value={value}
                      maxLength={60}
                      spellCheck={false}
                      autoComplete="off"
                      aria-label={t("setup.words.spelling", {
                        heard: term.heard ?? term.term,
                      })}
                      onChange={(e) => {
                        setInvalid(null);
                        setDrafts((d) => ({
                          ...d,
                          [term.key]: e.target.value,
                        }));
                      }}
                      onKeyDown={(e) => {
                        if (e.key === "Enter") {
                          e.preventDefault();
                          add(term);
                        } else if (e.key === "Escape" && value !== term.term) {
                          e.stopPropagation();
                          setDrafts((d) => ({ ...d, [term.key]: term.term }));
                        }
                      }}
                    />
                  ) : (
                    <b className="truncate text-sm font-medium">
                      {w?.text ?? entry?.text ?? term.term}
                    </b>
                  )}
                  <span
                    className={`line-clamp-2 text-[13px] leading-snug ${invalid === term.key ? "text-danger" : "text-text-3"}`}
                  >
                    {invalid === term.key
                      ? t("setup.words.typeIt")
                      : state === "pending"
                        ? same && term.heard
                          ? t("setup.words.rightThisTime")
                          : t("setup.words.notYet")
                        : state === "added"
                          ? w?.entryId
                            ? t("setup.words.addedOne")
                            : t("setup.words.inDictionary")
                          : state === "skipped"
                            ? same
                              ? t("setup.words.skipped")
                              : t("setup.words.skippedHeard", {
                                  heard: term.heard,
                                })
                            : entry?.hits
                              ? t("setup.words.knownHits", {
                                  count: entry.hits,
                                  hits: entry.hits.toLocaleString(
                                    i18n.language,
                                  ),
                                })
                              : t("setup.words.inDictionary")}
                  </span>
                  <span className="flex items-center justify-end gap-1">
                    {state === "pending" && (
                      <>
                        <Button
                          variant="secondary"
                          size="sm"
                          onClick={() => add(term)}
                        >
                          {t("setup.words.add")}
                        </Button>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() =>
                            setWord(term.key, {
                              state: "skipped",
                              text: term.term,
                            })
                          }
                        >
                          {t("setup.words.skip")}
                        </Button>
                      </>
                    )}
                    {(state === "added" || state === "known") && (
                      <span
                        className="flex h-5 w-5 items-center justify-center rounded-full bg-teal-600 text-white"
                        aria-hidden="true"
                      >
                        <Check className="h-3 w-3" />
                      </span>
                    )}
                    {(state === "added" || state === "skipped") && (
                      <Button
                        variant="ghost"
                        size="sm"
                        onClick={() => undo(term)}
                        aria-label={t("setup.words.undoLabel", {
                          word: w?.text ?? term.term,
                        })}
                      >
                        {t("setup.words.undo")}
                      </Button>
                    )}
                  </span>
                </div>
              );
            })}
          </div>
          {body && (
            <div className="mt-4 overflow-hidden rounded-[var(--r-lg)] border border-line">
              <div className="flex items-center gap-2 border-b border-line bg-surface-2 px-4 py-2 text-xs text-text-3">
                <AppGlyph scene="email" />
                {t("setup.words.preview")}
              </div>
              <div className="px-4 py-3 text-sm leading-relaxed">
                <SceneText text={body} marks={marks} animate={false} />
              </div>
            </div>
          )}
        </>
      }
    />
  );
};
