const baseWidth = 700;
const baseMinWidthRem = 34;
const minPlotWidth = 460;
const edgePadding = 12;
export const yLabelGap = 8;

// Budget conservatively for the 13-unit SVG system font at render time.
function labelWidth(label: string): number {
  let width = 0;
  for (const char of label) {
    if (" .,:;|!'-".includes(char)) width += 7;
    else if (/[a-z0-9]/.test(char) && char !== "m" && char !== "w") width += 10;
    else width += char.charCodeAt(0) < 128 ? 14 : 16;
  }
  return width;
}

function widest(labels: readonly string[]): number {
  return labels.reduce((width, label) => Math.max(width, labelWidth(label)), 0);
}

export function plotLayout(yLabels: readonly string[], xLabels: readonly string[], xTitle: string) {
  // Centered ticks need half their width outside their extreme plot position.
  const xInset = Math.ceil(widest(xLabels) / 2) + edgePadding;
  const left = Math.max(70, widest(yLabels) + yLabelGap + edgePadding, xInset);
  const rightInset = Math.max(25, xInset);
  const plotWidth = Math.max(minPlotWidth, labelWidth(xTitle));
  const width = Math.max(baseWidth, left + plotWidth + rightInset);
  return { width, left, right: width - rightInset, minWidthRem: width * baseMinWidthRem / baseWidth };
}

export function tickIndices(count: number): number[] {
  if (count < 2) return count === 1 ? [0] : [];
  const ticks = Math.min(count, 7);
  return Array.from({ length: ticks }, (_, index) => Math.round(index * (count - 1) / (ticks - 1)));
}
