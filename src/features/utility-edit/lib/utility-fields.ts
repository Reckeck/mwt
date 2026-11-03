/** The input of an item as the core stores it: the edit form's base and the model of `Refresh` (an item carries an id on top). */
export function utilityInputOf(item: UtilityItem): UtilityInput {
  return {
    name: item.name,
    active: item.active,
    kind: item.kind,
    currency: item.currency,
    property_id: item.property_id,
    store_id: item.store_id,
    category: item.category,
  }
}
