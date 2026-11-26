import type { ContrastPair } from './contrast'

const TEXT_MIN = 4.5
const GRAPHIC_MIN = 3

function pairs(foregrounds: readonly string[], backgrounds: readonly string[], min: number): ContrastPair[] {
  return foregrounds.flatMap((foreground) => backgrounds.map((background) => ({ foreground, background, min })))
}

const SURFACES = ['--background', '--card', '--popover'] as const
const AMOUNT_SURFACES = ['--background', '--card', '--muted'] as const

/** TC1: text on surfaces and on its own fills; borders and `--*-soft` fills are not gated. */
export const TEXT_PAIRS: readonly ContrastPair[] = [
  ...pairs(['--foreground', '--card-foreground', '--popover-foreground'], SURFACES, TEXT_MIN),
  ...pairs(['--muted-foreground'], ['--background', '--card'], TEXT_MIN),
  ...pairs(['--primary-foreground'], ['--primary'], TEXT_MIN),
  ...pairs(['--accent-foreground'], ['--accent'], TEXT_MIN),
  ...pairs(['--secondary-foreground'], ['--secondary'], TEXT_MIN),
  ...pairs(['--destructive-foreground'], ['--destructive'], TEXT_MIN),
  ...pairs(['--success-foreground'], ['--success'], TEXT_MIN),
  ...pairs(['--sidebar-foreground'], ['--sidebar'], TEXT_MIN),
  ...pairs(['--sidebar-primary-foreground'], ['--sidebar-primary'], TEXT_MIN),
  ...pairs(['--sidebar-accent-foreground'], ['--sidebar-accent'], TEXT_MIN),
]

/** TC2: money tokens and status colours used as amount text. */
export const AMOUNT_PAIRS: readonly ContrastPair[] = pairs(
  [
    '--money-income',
    '--money-expense',
    '--money-transfer',
    '--money-in-transit',
    '--success',
    '--destructive',
    '--warning',
  ],
  AMOUNT_SURFACES,
  TEXT_MIN,
)

/** TC3: chart series against the card (3:1, UI graphics). */
export const CHART_PAIRS: readonly ContrastPair[] = pairs(
  [
    '--chart-1',
    '--chart-2',
    '--chart-3',
    '--chart-4',
    '--chart-5',
    '--chart-actual',
    '--chart-bank',
    '--chart-crypto',
    '--chart-property',
    '--chart-accent',
    '--chart-income',
    '--chart-over',
  ],
  ['--card'],
  GRAPHIC_MIN,
)

export const CONTRAST_PAIRS: readonly ContrastPair[] = [...TEXT_PAIRS, ...AMOUNT_PAIRS, ...CHART_PAIRS]
