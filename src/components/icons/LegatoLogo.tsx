import legatoIcon from "../../assets/legato-icon.svg";

// A brand name, so it stays out of the translation files.
const APP_NAME = "Legato";

/**
 * The Legato app icon beside the name. `width` sets the overall size; the
 * icon and the text scale with it.
 */
const LegatoLogo = ({
  width = 120,
  className = "",
}: {
  width?: number;
  className?: string;
}) => {
  const fontSize = width / 6;
  return (
    <div
      className={`flex items-center ${className}`}
      style={{ gap: fontSize / 2 }}
    >
      <img
        src={legatoIcon}
        width={fontSize * 1.5}
        height={fontSize * 1.5}
        alt=""
        draggable={false}
      />
      <span
        className="font-semibold"
        style={{ fontSize, letterSpacing: "-0.01em" }}
      >
        {APP_NAME}
      </span>
    </div>
  );
};

export default LegatoLogo;
