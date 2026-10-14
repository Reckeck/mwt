export interface UtilityBillDialogProps {
  propertyId: string
  /** `YYYY-MM`; the current month when omitted. */
  period?: string
}

/** One line of the bill dialog, as the user edits it. */
export interface BillRow {
  line: UtilityBillLine
  include: boolean
  /** A one-rate meter's reading. */
  reading: string
  /** A multi-rate meter's readings by zone. */
  zoneReadings: Record<string, string>
  amount: string
  /** The readings last sent for pricing, so a late answer is dropped. */
  pricing?: string
}
