/** Building `TxFilter`s: every field is required on the wire, most are empty. */

export function emptyTxFilter(patch: Partial<TxFilter> = {}): TxFilter {
  return {
    scope: { store_ids: [], include_children: true },
    kinds: [],
    contact_id: null,
    flow_id: null,
    from: null,
    to: null,
    hide_internal: false,
    text: null,
    ...patch,
  }
}

/** One store, with or without its sub-accounts. */
export function storeTxFilter(storeId: string, includeChildren = true): TxFilter {
  return emptyTxFilter({ scope: { store_ids: [storeId], include_children: includeChildren } })
}

/** How many fields narrow the list — for a «Filters (3)» badge. */
export function activeFilterCount(filter: TxFilter): number {
  return [
    filter.scope.store_ids.length > 0,
    filter.kinds.length > 0,
    Boolean(filter.contact_id),
    Boolean(filter.flow_id),
    Boolean(filter.from || filter.to),
    filter.hide_internal,
    Boolean(filter.text?.trim()),
  ].filter(Boolean).length
}
