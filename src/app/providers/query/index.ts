// check-ui-ignore invalidate-all: an undo may revert any mutation, and the cache does not know its keys
import { MutationCache, QueryClient } from '@tanstack/vue-query'

import { type MutationNotice, notify, textOf } from '@/shared/lib/notify'

export { VueQueryPlugin } from '@tanstack/vue-query'

declare module '@tanstack/vue-query' {
  interface Register {
    mutationMeta: MutationNotice
  }
}

/**
 * Every action reports its outcome from here: a mutation names its success
 * text (and, if it can be taken back, how) in `meta`, and these handlers show
 * the toast — success with an optional "Отменить", failure with the reason.
 * A mutation without `meta` is left alone (`check-ui` makes sure there is none).
 */
const mutationCache = new MutationCache({
  onSuccess: (data, variables, _context, mutation) => {
    const meta = mutation.options.meta
    if (!meta?.success) return
    const built = typeof meta.success === 'function' ? meta.success(data, variables) : textOf(meta.success)
    const { text, description } = typeof built === 'string' ? { text: built, description: undefined } : built
    // An empty text means "nothing happened worth saying" (the user cancelled a file dialog).
    if (!text) return
    const undo = meta.undo
    notify.success(
      text,
      undo
        ? {
            description,
            action: {
              label: textOf('actions.undo'),
              onClick: () => {
                Promise.resolve(undo(data, variables))
                  .then(() => {
                    notify.info('toast.undone')
                    return queryClient.invalidateQueries()
                  })
                  .catch((err: unknown) => notify.error(err))
              },
            },
          }
        : { description, action: meta.action?.(data, variables) },
    )
  },
  onError: (err, _variables, _context, mutation) => {
    const meta = mutation.options.meta
    if (!meta || meta.silent) return
    notify.error(err, meta.error ?? 'toast.failed')
  },
})

/**
 * One client for the whole app, so every screen shares the same cache.
 *
 * Window focus is not a signal here: a Tauri window gains focus constantly, and
 * refetching on it made widgets reflow. Data changes arrive as backend events
 * (`prices-updated`, planning events) that invalidate the affected keys.
 */
export const queryClient = new QueryClient({
  mutationCache,
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
      // Query data is replaced, never edited in place (every `setQueryData` builds a new
      // value), so it needs no deep proxies: walking a page of transactions through them
      // was ~140× slower than the plain objects (rule 17).
      shallow: true,
      // Commands go to the local backend, not the network: offline they still answer
      // (from the vault and the cached rates), so nothing waits for the connection.
      networkMode: 'always',
    },
    mutations: { networkMode: 'always' },
  },
})
