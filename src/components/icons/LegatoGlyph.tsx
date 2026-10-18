/**
 * The Legato mark on its own, drawn in the current text color at the stroke
 * weight of the other sidebar icons.
 */
const LegatoGlyph = ({
  width = 24,
  height = 24,
  className,
}: {
  width?: number | string;
  height?: number | string;
  className?: string;
}) => (
  <svg
    width={width}
    height={height}
    viewBox="13 13 38 38"
    fill="none"
    stroke="currentColor"
    strokeWidth={3.25}
    strokeLinecap="round"
    strokeLinejoin="round"
    className={className}
    aria-hidden="true"
    xmlns="http://www.w3.org/2000/svg"
  >
    <path d="M16.25 38.75V27.5A5.25 5.25 0 0 1 26.75 27.5V42.5A5.25 5.25 0 0 0 37.25 42.5V21.5A5.25 5.25 0 0 1 47.75 21.5V30.75" />
  </svg>
);

export default LegatoGlyph;
