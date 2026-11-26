import type { WealthGroup } from '@/entities/report'

/** The `--chart-*` token behind each group: its slice in the donut (index in the palette) and its swatch (class, literal for Tailwind). */
export const GROUP_STYLE: Record<WealthGroup, { slice: number; swatch: string }> = {
  bank: { slice: 0, swatch: 'bg-chart-bank' },
  crypto: { slice: 1, swatch: 'bg-chart-crypto' },
  property: { slice: 2, swatch: 'bg-chart-property' },
  owed: { slice: 3, swatch: 'bg-chart-accent' },
  debts: { slice: 4, swatch: 'bg-destructive' },
}
