import type { LocationQuery } from 'vue-router'

const NEW_KEY = 'new'
const CATEGORY_KEY = 'category'

const first = (value: LocationQuery[string]): string => {
  const single = Array.isArray(value) ? value[0] : value
  return typeof single === 'string' ? single : ''
}

/** `?new=limit&category=<id>` asks the plan for a new limit; the category is optional. */
export function newLimitRequest(query: LocationQuery): { category: string } | null {
  return first(query[NEW_KEY]) === 'limit' ? { category: first(query[CATEGORY_KEY]) } : null
}

/** The query without the request, so a reload or back does not open the dialog again. */
export function withoutNewLimit(query: LocationQuery): LocationQuery {
  const { [NEW_KEY]: _new, [CATEGORY_KEY]: _category, ...rest } = query
  return rest
}
