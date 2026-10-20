import { type MaybeRefOrGetter, toValue } from 'vue'

/**
 * `placeholderData` for a query whose key holds an entity id and a filter
 * (a wallet and its period, a property and its month): the last data stays on
 * screen while the filter changes, but never under another entity's title —
 * switching to another wallet shows its own loading state (rule 17).
 */
export function keepPreviousFor(id: MaybeRefOrGetter<unknown>) {
  return <TData>(previous: TData | undefined, previousQuery?: { queryKey: readonly unknown[] }) =>
    previousQuery?.queryKey.includes(toValue(id)) ? previous : undefined
}
