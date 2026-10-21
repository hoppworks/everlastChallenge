import type { CSSProperties } from "react";

/** A shaped level indicator, not a frequency spectrum. Uses only native RMS. */
export function LevelVisual({ level }: { level?: number }) {
  if (level === undefined)
    return (
      <div className="level-visual" aria-hidden="true">
        <svg
          viewBox="0 0 24 24"
          fill="none"
          stroke="currentColor"
          strokeWidth="1.6"
        >
          <rect x="9" y="3" width="6" height="12" rx="3" />
          <path d="M5 10v2a7 7 0 0 0 14 0v-2M12 19v3m-4 0h8" />
        </svg>
      </div>
    );
  const value = Number.isFinite(level) ? Math.max(0, Math.min(1, level)) : 0;
  return (
    <div className="level-visual">
      <meter
        className="accessible-meter"
        min={0}
        max={1}
        value={value}
        aria-label="Mikrofonpegel"
      />
      <div className="level-bars" aria-hidden="true">
        {[0.4, 0.7, 0.9, 1, 0.8, 0.6, 0.35].map((weight, index) => (
          <i
            key={index}
            style={
              {
                "--bar-height": `${3 + value * weight * 23}px`,
              } as CSSProperties
            }
          />
        ))}
      </div>
    </div>
  );
}
