import type {
  AppCategory,
  CategoryStyle,
  PasteMethod,
  Styles,
  Tone,
} from "@/bindings";

/** The kinds of app, in the order the tabs show them. */
export const CATEGORIES: AppCategory[] = [
  "personal",
  "work",
  "email",
  "documents",
  "ai",
  "code",
  "other",
];

/** Mirrors `style_rules::tones_for`: three tones per kind of app. */
export const tonesFor = (category: AppCategory): Tone[] => {
  switch (category) {
    case "email":
      return ["formal", "casual", "excited"];
    case "documents":
      return ["formal", "casual", "notes"];
    case "ai":
      return ["raw", "light", "structured"];
    case "code":
      return ["raw", "lower", "sentence"];
    default:
      return ["formal", "casual", "lower"];
  }
};

/** Mirrors `style_rules::default_tone`. */
export const defaultTone = (category: AppCategory): Tone => {
  switch (category) {
    case "personal":
    case "other":
      return "casual";
    case "ai":
      return "light";
    case "code":
      return "lower";
    default:
      return "formal";
  }
};

/** A kind of app's style, with a tone from another kind replaced by this
 *  one's default, as the backend reads it. */
export const styleOf = (
  styles: Styles | undefined,
  category: AppCategory,
): CategoryStyle => {
  const style = styles?.[category] ?? { tone: defaultTone(category) };
  return tonesFor(category).includes(style.tone)
    ? style
    : { ...style, tone: defaultTone(category) };
};

export const asCategory = (value: string | null | undefined): AppCategory =>
  CATEGORIES.includes(value as AppCategory) ? (value as AppCategory) : "other";

/** Paste methods in the order the Paste with menu lists them. */
export const PASTE_METHODS: PasteMethod[] = [
  "ctrl_v",
  "ctrl_shift_v",
  "shift_insert",
  "direct",
  "none",
];
