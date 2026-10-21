/**
 * Property types offered in forms. `house` stays in the data of older vaults
 * and reads as a home, like `flat`; new records write `flat`.
 */
export const PROPERTY_CATEGORIES: PropertyCategory[] = ['flat', 'car', 'land', 'other']

/** The type a form shows: an old `house` is a home. */
export function shownPropertyCategory(category: PropertyCategory | null | undefined): PropertyCategory {
  return !category || category === 'house' ? 'flat' : category
}

/** A home: the only property with utilities, meters and a pay day. */
export function isHome(category: PropertyCategory | null | undefined): boolean {
  return category === 'flat' || category === 'house'
}
