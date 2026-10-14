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

type Query = Record<string, string | null | (string | null)[] | undefined>

/** The filter as short query keys, only what is set: `?store=…&kind=payment&q=кофе`. */
export function txFilterToQuery(filter: TxFilter): Record<string, string | undefined> {
  return {
    store: filter.scope.store_ids[0] || undefined,
    children: filter.scope.store_ids.length && !filter.scope.include_children ? '0' : undefined,
    kind: filter.kinds[0] || undefined,
    contact: filter.contact_id || undefined,
    flow: filter.flow_id || undefined,
    from: filter.from || undefined,
    to: filter.to || undefined,
    internal: filter.hide_internal ? 'hide' : undefined,
    q: filter.text?.trim() || undefined,
  }
}

/** The query keys a filter may use. */
export const TX_FILTER_QUERY_KEYS = ['store', 'children', 'kind', 'contact', 'flow', 'from', 'to', 'internal', 'q']

/** Back from the query; `null` when the address carries no filter at all. */
export function txFilterFromQuery(query: Query): TxFilter | null {
  const one = (key: string) => {
    const value = query[key]
    const first = Array.isArray(value) ? value[0] : value
    return typeof first === 'string' && first ? first : null
  }
  if (!TX_FILTER_QUERY_KEYS.some((key) => one(key))) return null
  const store = one('store')
  const kind = one('kind')
  return emptyTxFilter({
    scope: { store_ids: store ? [store] : [], include_children: one('children') !== '0' },
    kinds: kind ? [kind as TxKind] : [],
    contact_id: one('contact'),
    flow_id: one('flow'),
    from: one('from'),
    to: one('to'),
    hide_internal: one('internal') === 'hide',
    text: one('q'),
  })
}
