export const FILTER_KINDS: TxKind[] = [
  'payment',
  'topup',
  'transfer',
  'convert',
  'trade_spot',
  'trade_futures',
  'defi',
  'asset_sell',
  'adjustment',
]

type SwitchField = 'auto_only' | 'debt_only' | 'with_attachments' | 'hide_internal' | 'unpriced' | 'with_fee'

/** The on/off filters of the sheet: the field and its label key. */
export const FILTER_SWITCHES: { field: SwitchField; label: string }[] = [
  { field: 'auto_only', label: 'filter.autoOnly' },
  { field: 'debt_only', label: 'filter.debtOnly' },
  { field: 'with_attachments', label: 'filter.withAttachments' },
  { field: 'hide_internal', label: 'filter.hideInternal' },
  { field: 'unpriced', label: 'filter.unpriced' },
  { field: 'with_fee', label: 'filter.withFee' },
]
