import React from "react";

// The few icons the island and the bubble draw themselves, so each can move
// the way its button works (styles/icons.css): Dictate is a level meter
// that rises and falls, Notes a pen writing a line, the Scratchpad a page
// filling with lines. They stay still at rest and with reduced motion.

/** Each bar's height at rest, the two heights it swings between under the
 *  pointer, how long a swing takes and how late it starts. */
const BARS = [
  { x: 2, rest: 0.34, lo: 0.25, hi: 0.7, ms: 330, late: 0 },
  { x: 5, rest: 0.7, lo: 0.35, hi: 1, ms: 270, late: 90 },
  { x: 8, rest: 1, lo: 0.45, hi: 1, ms: 390, late: 30 },
  { x: 11, rest: 0.56, lo: 0.3, hi: 0.9, ms: 300, late: 140 },
  { x: 14, rest: 0.3, lo: 0.22, hi: 0.62, ms: 360, late: 60 },
];

/** Dictate: the meter a dictation shows, at rest. */
export const VoiceGlyph: React.FC = () => (
  <svg className="g g-voice" viewBox="0 0 16 16" aria-hidden="true">
    {BARS.map((bar) => (
      <rect
        key={bar.x}
        x={bar.x - 0.8}
        y="2"
        width="1.6"
        height="12"
        rx="0.8"
        style={
          {
            "--rest": bar.rest,
            "--lo": bar.lo,
            "--hi": bar.hi,
            "--ms": `${bar.ms}ms`,
            "--late": `${bar.late}ms`,
          } as React.CSSProperties
        }
      />
    ))}
  </svg>
);

/** Notes: a pen at the end of the line it wrote. */
export const PenGlyph: React.FC = () => (
  <svg className="g g-pen" viewBox="0 0 16 16" aria-hidden="true">
    <path
      className="g-ink"
      pathLength={1}
      d="M1.5 13.3c1-1.6 2-1.6 3 0s2 1.6 3 0"
    />
    <path
      className="g-nib"
      d="M7.9 12.5l.8-2.8 4.8-4.8a1.3 1.3 0 0 1 1.9 1.9l-4.8 4.8z"
    />
  </svg>
);

/** The Scratchpad: a page with a few lines on it. */
export const PadGlyph: React.FC = () => (
  <svg className="g g-pad" viewBox="0 0 16 16" aria-hidden="true">
    <rect x="2.75" y="1.75" width="10.5" height="12.5" rx="2.2" />
    <path className="g-line" pathLength={1} d="M5.5 5.4h5" />
    <path className="g-line" pathLength={1} d="M5.5 8h5" />
    <path className="g-line" pathLength={1} d="M5.5 10.6h2.8" />
  </svg>
);
