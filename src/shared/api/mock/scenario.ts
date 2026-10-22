/**
 * Which data the browser mock plays: `?mock=<name>` in the address, or
 * `localStorage['mock-scenario']`, so screenshots can show every state of a screen.
 *
 * - `year` (default): twelve months of everyday spending on top of the demo history;
 * - `empty`: a new vault, no wallets and no history;
 * - `stress`: the year in millions, long names, a currency without a rate;
 * - `slow`: the year, answering after 2.5 s (loading states);
 * - `error`: every command past unlock fails (error states).
 */

export type MockScenario = 'year' | 'empty' | 'stress' | 'slow' | 'error'

const SCENARIOS: readonly MockScenario[] = ['year', 'empty', 'stress', 'slow', 'error']

function readScenario(): MockScenario {
  let wanted: string | null = null
  try {
    wanted = new URLSearchParams(globalThis.location?.search ?? '').get('mock')
    wanted ??= globalThis.localStorage?.getItem('mock-scenario') ?? null
  } catch {
    // No storage (private window): the default.
  }
  return SCENARIOS.find((name) => name === wanted) ?? 'year'
}

/** Read once: switching the scenario reloads the page. */
export const mockScenario: MockScenario = readScenario()

/** Reply delay for the scenario; check-layout's `__mockLatency` still wins. */
export const scenarioLatency = mockScenario === 'slow' ? 2500 : 30
