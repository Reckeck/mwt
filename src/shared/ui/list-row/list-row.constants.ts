import { MONEY_FLOW_CLASS } from '@/shared/lib/money-flow'

import type { ListRowTone } from './list-row.types'

/** Ink and positive are the `money-flow` expense and income colours; negative is the only extra. */
export const LIST_ROW_TONE_CLASS: Record<ListRowTone, string> = {
  ink: MONEY_FLOW_CLASS.expense,
  positive: MONEY_FLOW_CLASS.income,
  negative: 'text-destructive',
}
