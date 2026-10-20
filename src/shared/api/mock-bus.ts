/**
 * Backend events in `pnpm dev`: the in-memory mock emits them here and
 * `onEvent` listens here when there is no Tauri. Kept out of `mock/` so the
 * mock itself still loads lazily.
 */
type Handler = (payload: unknown) => void

const handlers = new Map<string, Set<Handler>>()

export function listenMock(event: string, handler: Handler): () => void {
  const set = handlers.get(event) ?? new Set<Handler>()
  set.add(handler)
  handlers.set(event, set)
  return () => set.delete(handler)
}

export function emitMock(event: string, payload: unknown) {
  for (const handler of handlers.get(event) ?? []) handler(payload)
}

/** How many handlers listen to `event` now: `check-layout` uses it to catch listeners that leak per visit. */
export function mockListenerCount(event: string): number {
  return handlers.get(event)?.size ?? 0
}
