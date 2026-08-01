import type { CSSProperties } from "react";

const MIN_CONTRAST = 4.5;

function relativeLuminance(hex: string): number {
  const channels = [0, 1, 2].map(
    (offset) => Number.parseInt(hex.slice(1 + offset * 2, 3 + offset * 2), 16) / 255,
  );
  const linear = channels.map((channel) =>
    channel <= 0.03928 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4,
  );
  return 0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2];
}

function contrastRatio(a: number, b: number): number {
  const [high, low] = a >= b ? [a, b] : [b, a];
  return (high + 0.05) / (low + 0.05);
}

/** Return safe inline colors for a GitHub label, or undefined for unsafe input. */
export function issueLabelStyle(color: string | null | undefined): CSSProperties | undefined {
  if (!color || !/^[0-9a-f]{6}$/i.test(color)) return undefined;

  const background = `#${color.toLowerCase()}`;
  const backgroundLuminance = relativeLuminance(background);
  const foreground = ["#ffffff", "#111827"].find((candidate) =>
    contrastRatio(relativeLuminance(candidate), backgroundLuminance) >= MIN_CONTRAST,
  );
  if (!foreground) return undefined;

  return {
    backgroundColor: background,
    borderColor: background,
    color: foreground,
  };
}
