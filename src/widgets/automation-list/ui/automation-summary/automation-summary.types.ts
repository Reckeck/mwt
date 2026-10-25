import type { SumPart } from '@/shared/lib/money-parts'

export interface AutomationSummaryProps {
  automation: Automation
  /** «Пополнение · ~1 000 USD · Карта»: what one run books. */
  line: SumPart[]
  /** Refused runs waiting for «Повторить / Пропустить». */
  rejected: number
  /** Operations booked approximately, waiting for the real amount. */
  unconfirmed: number
}
