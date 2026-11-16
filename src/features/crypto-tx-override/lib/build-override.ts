import type { OverrideRequest } from '../model/use-crypto-override'

export interface MeaningFields {
  category: string
  description: string
  contact: string
}

/**
 * Only what the user changed is sent: an untouched field is not a correction, and sending a category that a
 * rule had set would turn it into a hand-made override that the rule can no longer move. «Apply to this
 * contract» is the exception: it lays the category and the counterparty over the other operations, so both go.
 */
export function buildOverride(row: CryptoOpRow, fields: MeaningFields, wide: boolean): OverrideRequest {
  const input: OverrideRequest = { event_id: row.id, same_contract: wide }
  const clear: OverrideField[] = []

  const category = row.categories[0] ?? ''
  if (fields.category) {
    if (wide || fields.category !== category) input.categories = [fields.category]
  } else if (category) clear.push('categories')

  const text = fields.description.trim()
  if (text !== (row.comment ?? '')) {
    if (text) input.comment = text
    else clear.push('comment')
  }

  const contact = row.contact_id ?? ''
  if (fields.contact) {
    if (wide || fields.contact !== contact) input.contact_id = fields.contact
  } else if (contact) clear.push('contact')

  return { ...input, clear }
}
