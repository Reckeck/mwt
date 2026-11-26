/** How many heat steps there are: `--chart-heat-0` (quiet) … `--chart-heat-4` (the month's heaviest day). */
export const HEAT_LEVELS = 5

/** Background (kept on hover) and ink of a cell by heat step; the tokens flip with the theme, so does the ink. */
export const HEAT_CLASSES = [
  'bg-chart-heat-0 hover:bg-chart-heat-0',
  'bg-chart-heat-1 hover:bg-chart-heat-1',
  'bg-chart-heat-2 hover:bg-chart-heat-2',
  'bg-chart-heat-3 hover:bg-chart-heat-3 text-background',
  'bg-chart-heat-4 hover:bg-chart-heat-4 text-background',
] as const
