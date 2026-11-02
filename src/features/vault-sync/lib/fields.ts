/** How the fields of an edited record read in the list of sync changes. */

export interface FieldView {
  /** Translated field name, or a humanized one. */
  label: string
  before: string | null
  after: string | null
  /** Set when both sides are amounts of this lowercase currency or asset: the row uses `<Money>`. */
  currency: string | null
}

type Translate = (key: string) => string
type Exists = (key: string) => boolean

/** `meta.comment` -> `Meta comment`: all the label a field gets when no text is written for it. */
export function humanizeField(field: string): string {
  const spaced = field.replace(/[._-]+/g, ' ').trim()
  return spaced ? spaced.charAt(0).toUpperCase() + spaced.slice(1) : field
}

export function fieldLabel(field: string, t: Translate, te: Exists): string {
  const key = `sync.field.${field}`
  return te(key) ? t(key) : humanizeField(field)
}

export function describeField(change: FieldChange, t: Translate, te: Exists): FieldView {
  return {
    label: fieldLabel(change.field, t, te),
    before: change.before,
    after: change.after,
    currency: change.money_currency,
  }
}
