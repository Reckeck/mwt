// check-ui-ignore raw-color: store colours are user data, saved in the vault as hex
export const DEFAULT_STORE_COLOR = '#0ea5e9'

/** Default accent per kind, so groups in the sidebar read at a glance. */
export const KIND_COLORS: Record<StoreKind, string> = {
  cash: '#16a34a',
  bank: '#2563eb',
  crypto: '#f59e0b',
  exchange: '#eab308',
  property: '#7c3aed',
  loyalty: '#db2777',
  liability: '#dc2626',
}

/** A choice card (wallet kind, sub-account type, exchange): an outline `Button` laid out as a card. */
export const PICK_CARD_CLASS = 'h-auto items-start justify-start gap-3 whitespace-normal p-3 text-left font-normal'
