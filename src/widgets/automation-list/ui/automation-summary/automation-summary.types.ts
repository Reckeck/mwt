export interface AutomationSummaryProps {
  automation: Automation
  /** «Пополнение · ~1 000 USD · Карта»: what one run books. */
  line: string
  /** Refused runs waiting for «Повторить / Пропустить». */
  rejected: number
  /** Operations booked approximately, waiting for the real amount. */
  unconfirmed: number
}
