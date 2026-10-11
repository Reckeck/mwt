import { QueryClient } from '@tanstack/vue-query'

export { VueQueryPlugin } from '@tanstack/vue-query'

/**
 * One client for the whole app, so every screen shares the same cache.
 *
 * Window focus is not a signal here: a Tauri window gains focus constantly, and
 * refetching on it made widgets reflow. Data changes arrive as backend events
 * (`prices-updated`, planning events) that invalidate the affected keys.
 */
export const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      refetchOnWindowFocus: false,
    },
  },
})
