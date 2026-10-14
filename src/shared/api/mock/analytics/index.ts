/**
 * Browser-mock half of analytics: reports and the calendar for `pnpm dev`.
 * Rough sums over the mock ledger in USD, enough to lay the screens out;
 * the real reports (rates by date, real terms, past valuations) live only in
 * the backend.
 */

export type { AnalyticsMockContext } from './flows'
export * from './entry'
