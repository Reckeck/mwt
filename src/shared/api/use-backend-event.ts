import { onScopeDispose } from 'vue'

import { onEvent } from './ipc'

/**
 * Listen to a backend event for as long as the calling component (or effect
 * scope) lives; the listener is removed with it, so a remounted page or a hot
 * reload never ends up handling the same event twice.
 */
export function useBackendEvent<T>(event: string, handler: (payload: T) => void): void {
  const unlisten = onEvent<T>(event, handler)
  onScopeDispose(() => {
    void unlisten.then((stop) => stop())
  })
}
