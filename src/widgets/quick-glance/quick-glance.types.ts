export interface SpentGlanceProps {
  /** `YYYY-MM`. */
  month: string
  /** The screen the glance is on, as the operations list's «←» names it. */
  backLabel: string
  triggerClass?: string
}

export interface CapitalGlanceProps {
  triggerClass?: string
}

export interface WalletGlanceProps {
  storeId: string
  /** The wallet's name as the row shows it («Bybit › Futures»). */
  name: string
  triggerClass?: string
}

export interface CategoryGlanceProps {
  /** The category's key. */
  category: string
  /** `YYYY-MM`. */
  month: string
}

export interface WalletGlanceBodyProps {
  storeId: string
  name: string
}

export interface AutomationGlanceProps {
  /** The operation the automation booked. */
  tx: Transaction
  /** The automation's id (a workflow's, for a record a workflow made). */
  automationId: string
  runId: string | null
}
