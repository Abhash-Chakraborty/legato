import { useEffect, useState } from "react";
import { commands, events, type SpeakerModel } from "@/bindings";

/** The speaker model on this PC, kept current while a download runs. */
export const useSpeakerModel = (): SpeakerModel | null => {
  const [model, setModel] = useState<SpeakerModel | null>(null);
  useEffect(() => {
    let live = true;
    commands
      .getSpeakerModel()
      .then((m) => live && setModel(m))
      .catch(() => {});
    const stop = events.speakerModelChanged.listen(
      (e) => live && setModel(e.payload.model),
    );
    return () => {
      live = false;
      stop.then((f) => f()).catch(() => {});
    };
  }, []);
  return model;
};
