/**
 * Stage 3 · DeFi positions (spec D5, canvas page «14 DeFi»): the list, the detail of a position and the
 * filtered list. The mock holds the reference positions of `cryptotrack::protocols::fees` (a pool, a supply
 * with a debt, a stake, a position whose fees nobody could read, and a closed one), so the figures on the
 * screenshot are the same arithmetic the application computes.
 */

const SETTLE = 900

export const defi = [
  { name: 'defi-list', route: '/crypto/positions', steps: [{ wait: SETTLE }] },
  {
    name: 'defi-detail',
    route: '/crypto/positions',
    phone: false,
    steps: [{ wait: SETTLE }, { click: 'tbody tr' }, { wait: 700 }],
  },
  {
    name: 'defi-detail-sheet',
    route: '/crypto/positions',
    desktop: false,
    steps: [{ wait: SETTLE }, { tap: 'li button' }, { wait: 700 }],
  },
]
