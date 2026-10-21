/** Which property an operation belongs to, guessed from its categories (plan G4). */

/** Categories that point at a kind of property; the guess holds only when one such property exists. */
const PROPERTY_CATEGORIES: Record<string, PropertyCategory[]> = {
  rent_income: ['flat', 'house'],
  fuel: ['car'],
  insurance: ['car'],
  repair: ['car'],
}

/** The one live property the categories point at; `''` when none or several fit. */
export function suggestedProperty(categories: string[], properties: Store[]): string {
  const kinds = new Set(categories.flatMap((key) => PROPERTY_CATEGORIES[key] ?? []))
  if (!kinds.size) return ''
  const fitting = properties.filter((store) => kinds.has(store.property?.category ?? 'other'))
  return fitting.length === 1 ? (fitting[0]?.id ?? '') : ''
}
