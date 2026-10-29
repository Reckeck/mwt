import type { SumPart } from '@/shared/lib/money-parts'

export interface AutomationSummaryProps {
  automation: Automation
  /** `Top-up · ~1,000 USD · Card`: what one run books. */
  line: SumPart[]
  /** Refused runs waiting for `Retry / Skip`. */
  rejected: number
  /** Operations booked approximately, waiting for the real amount. */
  unconfirmed: number
}
