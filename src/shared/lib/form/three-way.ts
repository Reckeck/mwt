const isRecord = (value: unknown): value is Record<string, unknown> =>
  typeof value === 'object' && value !== null && !Array.isArray(value)

/** Deep equality of plain data (objects, arrays, scalars), independent of key order. */
export function sameData(left: unknown, right: unknown): boolean {
  if (left === right) return true
  if (Array.isArray(left) && Array.isArray(right)) {
    return left.length === right.length && left.every((item, index) => sameData(item, right[index]))
  }
  if (isRecord(left) && isRecord(right)) {
    const keys = new Set([...Object.keys(left), ...Object.keys(right)])
    return [...keys].every((key) => sameData(left[key], right[key]))
  }
  return false
}

/**
 * Three-way merge of a form: `base` is what the form opened with, `current` what the user
 * has now, `fresh` what the record is now. A field the user did not touch takes the fresh
 * value; objects merge field by field; arrays and scalars go whole; a field changed on both
 * sides keeps the user's edit (the same rule as `rebase.rs` in the core).
 */
export function threeWay<Model>(base: Model, current: Model, fresh: Model): Model {
  if (isRecord(base) && isRecord(current) && isRecord(fresh)) {
    const keys = new Set([...Object.keys(base), ...Object.keys(current), ...Object.keys(fresh)])
    const merged: Record<string, unknown> = {}
    for (const key of keys) merged[key] = threeWay(base[key], current[key], fresh[key])
    return merged as Model
  }
  return sameData(current, base) ? fresh : current
}
