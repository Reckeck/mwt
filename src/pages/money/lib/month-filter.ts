import { shiftMonth } from '@/shared/lib/period'

/**
 * The balance report for a month: the twelve months ending with it over every wallet, so a carry from
 * earlier months reaches the one shown.
 */
export function monthBalanceFilter(month: string): ReportFilter {
  return {
    scope: { store_ids: [], include_children: false },
    from: shiftMonth(month, -11),
    to: month,
    year: null,
    currency: null,
    real_terms: false,
  }
}
