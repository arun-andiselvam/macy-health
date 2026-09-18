// Illustrations for each reminder kind. All motion lives in illustrations.css.
import "./illustrations.css";

type Props = {
  kind: string;
  emoji: string;
  /** Eye-rest countdown ring length. */
  countdownMs?: number;
  /** Static pose, for lists. */
  still?: boolean;
};

export function Illustration({ kind, emoji, countdownMs = 20_000, still = false }: Props) {
  const variant = still ? " art-still" : "";
  switch (kind) {
    case "water":
      return <WaterGlass variant={variant} />;
    case "eyes":
      return <RestingEye variant={variant} countdownMs={countdownMs} />;
    case "stand":
      return <StandingFigure variant={variant} />;
    default:
      return <span className={`emoji${still ? " emoji-still" : ""}`}>{emoji}</span>;
  }
}

// ---------- water: a drop falls and the glass fills ----------

const GLASS = "M18.5 14.5 L45.5 14.5 L41.8 55 Q41.6 57 39.5 57 L24.5 57 Q22.4 57 22.2 55 Z";

// Water body whose top edge is a wave (period 16) at local y=0.
const WAVE = (() => {
  let d = "M-16 0";
  for (let x = -16; x < 96; x += 16) d += " q4 -2.2 8 0 t8 0";
  return `${d} V64 H-16 Z`;
})();

function WaterGlass({ variant }: { variant: string }) {
  return (
    <svg className={`art art-water${variant}`} viewBox="0 0 64 64" aria-hidden="true">
      <defs>
        <clipPath id="glass-clip">
          <path d={GLASS} />
        </clipPath>
      </defs>
      <g clipPath="url(#glass-clip)">
        <g className="water-level">
          <path className="water-back" d={WAVE} />
          <path className="water-front" d={WAVE} />
          <circle className="bubble b1" cx="27" cy="18" r="1.3" />
          <circle className="bubble b2" cx="35" cy="22" r="1" />
          <circle className="bubble b3" cx="31" cy="20" r="1.6" />
        </g>
      </g>
      <path className="glass" d={GLASS} />
      <path className="drop" d="M32 1.5 C32 1.5 28.2 6.2 28.2 8.6 A3.8 3.8 0 0 0 35.8 8.6 C35.8 6.2 32 1.5 32 1.5 Z" />
    </svg>
  );
}

// ---------- eyes: blinking eye that looks into the distance, countdown ring ----------

const EYE = "M12 32 Q32 14 52 32 Q32 50 12 32 Z";

function RestingEye({ variant, countdownMs }: { variant: string; countdownMs: number }) {
  return (
    <svg className={`art art-eyes${variant}`} viewBox="0 0 64 64" aria-hidden="true">
      <circle className="ring-track" cx="32" cy="32" r="29" />
      <circle
        className="ring"
        cx="32"
        cy="32"
        r="29"
        pathLength={100}
        transform="rotate(-90 32 32)"
        style={{ animationDuration: `${countdownMs}ms` }}
      />
      <defs>
        <clipPath id="eye-clip">
          <path d={EYE} />
        </clipPath>
      </defs>
      <g className="eye">
        <path className="eye-white" d={EYE} />
        <g clipPath="url(#eye-clip)">
          <g className="iris">
            <circle className="iris-color" cx="32" cy="32" r="8.5" />
            <circle className="pupil" cx="32" cy="32" r="3.8" />
            <circle className="glint" cx="34.6" cy="29.4" r="1.7" />
          </g>
        </g>
        <path className="eye-outline" d={EYE} />
      </g>
    </svg>
  );
}

// ---------- stand: figure raises arms and stretches ----------

function StandingFigure({ variant }: { variant: string }) {
  return (
    <svg className={`art art-stand${variant}`} viewBox="0 0 64 64" aria-hidden="true">
      <g className="sparkles">
        <path d="M18 12 L14 9" />
        <path d="M46 12 L50 9" />
        <path d="M32 4.5 V1" />
      </g>
      <g className="figure">
        <path className="limb arm arm-left" d="M32 24 V37" />
        <path className="limb arm arm-right" d="M32 24 V37" />
        <path className="limb" d="M32 20 V38 M32 38 L26 55 M32 38 L38 55" />
        <circle className="head" cx="32" cy="13" r="5.5" />
      </g>
      <path className="ground" d="M19 58.5 H45" />
    </svg>
  );
}
