/**
 * Known `INVALID_INPUT` messages of the core (`service/validate.rs`) that point
 * at a single field: the form shows them under it instead of a generic toast.
 * Unknown messages stay a toast, with the core's text appended (`vault-error`).
 */

export interface CoreFieldError {
  /** The form field the message belongs under. */
  field: string
  /** i18n key of the text shown under it. */
  key: string
}

const RULES: { match: string; kinds: TxKind[]; error: CoreFieldError }[] = [
  {
    match: 'a conversion needs two different assets',
    kinds: ['convert'],
    error: { field: 'toAsset', key: 'validation.sameCurrency' },
  },
  {
    match: 'a transfer needs two different sides',
    kinds: ['transfer'],
    error: { field: 'to', key: 'validation.pickOtherWallet' },
  },
  {
    match: 'a trade needs two different assets',
    kinds: ['trade_spot'],
    error: { field: 'boughtAsset', key: 'validation.pickOtherAsset' },
  },
  {
    match: 'both sides land on the same wallet and asset',
    kinds: ['convert', 'transfer'],
    error: { field: 'to', key: 'validation.pickOtherWallet' },
  },
]

/** The field a core message of a `kind` form belongs under, or `null` for a message we do not know. */
export function coreErrorField(message: string, kind: TxKind): CoreFieldError | null {
  return RULES.find((rule) => rule.kinds.includes(kind) && message.includes(rule.match))?.error ?? null
}
