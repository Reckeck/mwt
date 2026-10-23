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
    category: null,
    with_fee: false,
    realized: false,
    amount_min: null,
    amount_max: null,
    auto_only: false,
    debt_only: false,
    with_attachments: false,
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
    filter.category !== null,
    filter.with_fee,
    filter.realized,
    Boolean(filter.amount_min || filter.amount_max),
    filter.auto_only,
    filter.debt_only,
    filter.with_attachments,
  ].filter(Boolean).length
}

type Query = Record<string, string | null | (string | null)[] | undefined>

/** The filter as short query keys, only what is set: `?store=a,b&kind=payment&q=кофе`. */
export function txFilterToQuery(filter: TxFilter): Record<string, string | undefined> {
  return {
    store: filter.scope.store_ids.join(',') || undefined,
    children: filter.scope.store_ids.length && !filter.scope.include_children ? '0' : undefined,
    kind: filter.kinds.join(',') || undefined,
    contact: filter.contact_id || undefined,
    flow: filter.flow_id || undefined,
    from: filter.from || undefined,
    to: filter.to || undefined,
    internal: filter.hide_internal ? 'hide' : undefined,
    q: filter.text?.trim() || undefined,
    // `''` is «without a category», a filter of its own.
    cat: filter.category ?? undefined,
    fee: filter.with_fee ? '1' : undefined,
    realized: filter.realized ? '1' : undefined,
    min: filter.amount_min || undefined,
    max: filter.amount_max || undefined,
    auto: filter.auto_only ? '1' : undefined,
    debt: filter.debt_only ? '1' : undefined,
    files: filter.with_attachments ? '1' : undefined,
  }
}

/** The query keys a filter may use. */
export const TX_FILTER_QUERY_KEYS = [
  'store',
  'children',
  'kind',
  'contact',
  'flow',
  'from',
  'to',
  'internal',
  'q',
  'cat',
  'fee',
  'realized',
  'min',
  'max',
  'auto',
  'debt',
  'files',
]

/** Back from the query; `null` when the address carries no filter at all. */
export function txFilterFromQuery(query: Query): TxFilter | null {
  const one = (key: string) => {
    const value = query[key]
    const first = Array.isArray(value) ? value[0] : value
    return typeof first === 'string' && first ? first : null
  }
  const category = typeof query.cat === 'string' ? query.cat : null
  if (!TX_FILTER_QUERY_KEYS.some((key) => one(key)) && category === null) return null
  // Lists are comma-separated; an old single value reads as a list of one.
  const list = (key: string) => (one(key) ?? '').split(',').filter(Boolean)
  return emptyTxFilter({
    scope: { store_ids: list('store'), include_children: one('children') !== '0' },
    kinds: list('kind') as TxKind[],
    contact_id: one('contact'),
    flow_id: one('flow'),
    from: one('from'),
    to: one('to'),
    hide_internal: one('internal') === 'hide',
    text: one('q'),
    category,
    with_fee: one('fee') === '1',
    realized: one('realized') === '1',
    amount_min: one('min'),
    amount_max: one('max'),
    auto_only: one('auto') === '1',
    debt_only: one('debt') === '1',
    with_attachments: one('files') === '1',
  })
}
