import { create } from "zustand";
import { toast } from "sonner";
import i18n from "../i18n";
import { commands, events, type CallState, type CallTest } from "@/bindings";
import { useNavStore } from "./navStore";

/** Calls Legato noticed, for the Notes page and Settings › Context & calls. */
interface CallsStore {
  state: CallState | null;
  load: () => Promise<void>;
}

export const useCallsStore = create<CallsStore>()((set) => ({
  state: null,
  load: async () => {
    const state = await commands.getCallState().catch(() => null);
    if (state) set({ state });
  },
}));

/** What a button on the test card would have done in a real call. */
const testToast = ({ action, snooze_minutes }: CallTest) => {
  if (action === "start") {
    const onNotes = useNavStore.getState().page === "notes";
    toast(i18n.t("calls.test.started"), {
      action: onNotes
        ? undefined
        : {
            label: i18n.t("calls.test.openNotes"),
            onClick: () => useNavStore.getState().go("notes"),
          },
    });
  } else if (action === "not_now") {
    toast(
      snooze_minutes
        ? i18n.t("calls.test.snoozed", { count: snooze_minutes })
        : i18n.t("calls.test.snoozedCall"),
    );
  }
};

let listening = false;

/** Loads the call state once and keeps it current. */
export const watchCalls = () => {
  const { state, load } = useCallsStore.getState();
  if (!state) load();
  if (listening) return;
  listening = true;
  Promise.all([
    events.callsChanged.listen((e) =>
      useCallsStore.setState({ state: e.payload.state }),
    ),
    events.callTest.listen((e) => testToast(e.payload)),
  ]).catch(() => {
    listening = false;
  });
};
