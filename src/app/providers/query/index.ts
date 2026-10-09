import { QueryClient } from '@tanstack/vue-query'

export { VueQueryPlugin } from '@tanstack/vue-query'

/** One client for the whole app, so every screen shares the same cache. */
export const queryClient = new QueryClient()
