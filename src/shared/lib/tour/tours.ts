import type { TourStep } from './tours.types.ts'

export type { TourStep } from './tours.types.ts'

/**
 * Tours by route name. Steps whose element is not on the screen are skipped. Only the screens that
 * survived the T72 deletion keep a tour; the rest are in `PLACEHOLDER_ROUTES` until their W4 task
 * brings the screen and its tour back (the `check-onboarding` gate reads both, see the `onboarding` skill).
 */
export const TOURS: Record<string, TourStep[]> = {
  // The shell around the placeholder body: the bottom bar, the quick add, the sidebar, the palette, the profile menu.
  dashboard: [
    // Only on a phone.
    { target: 'bottom-nav', key: 'dashboard.bottomNav', side: 'top' },
    { target: 'quick-add', key: 'dashboard.quickAdd', side: 'bottom' },
    { target: 'sidebar-toggle', key: 'dashboard.sidebarToggle', side: 'right' },
    { target: 'command-palette', key: 'dashboard.palette', side: 'bottom' },
    { target: 'open-settings', key: 'dashboard.settings', side: 'right' },
    { target: 'profile-trigger', key: 'dashboard.contextMenu', side: 'bottom' },
  ],
  // Capital: the accounts screen and Money · Capital share the hero and the allocation.
  stores: [
    { target: 'wealth-head', key: 'money.wealthHead', side: 'bottom' },
    { target: 'wealth-table', key: 'money.wealthTable', side: 'top' },
    { target: 'stores-list', key: 'stores.list', side: 'top' },
  ],
  'money-wealth': [
    { target: 'wealth-head', key: 'money.wealthHead', side: 'bottom' },
    { target: 'wealth-table', key: 'money.wealthTable', side: 'top' },
  ],
  'money-investments': [
    { target: 'invest-figures', key: 'money.investFigures', side: 'bottom' },
    { target: 'invest-table', key: 'money.investTable', side: 'top' },
  ],
  'crypto-positions': [
    { target: 'defi-list', key: 'defi.list', side: 'top' },
    { target: 'defi-net', key: 'defi.net', side: 'bottom' },
    { target: 'defi-fees', key: 'defi.fees', side: 'top' },
  ],
  // The settings window opens over any screen; a step of a tab that is not open is skipped.
  settings: [{ target: 'appearance-settings', key: 'settings.appearance', side: 'bottom' }],
  // Before a vault is open: the sandbox.
  unlock: [{ target: 'unlock-sandbox', key: 'unlock.sandbox', side: 'top' }],
}

/** Routes whose body is the shared placeholder until the W4 task that rebuilds the screen (spec D14). */
const PLACEHOLDER_ROUTES = [
  'money',
  'money-where',
  'money-plan',
  'money-month',
  'money-health',
  'money-calculator',
  'money-together',
  'money-together-event',
  'wallet',
  'wallet-settings',
  'transactions',
  'transactions-calendar',
  'transactions-bonuses',
  'contacts',
  'crypto-operations',
  'automations',
  'automations-journal',
  'automations-workflows',
  'automations-reminders',
  'automations-alerts',
  'automations-sources',
  'workflow-new',
  'workflow-edit',
]

/** Routes with no tour, and why (the `check-onboarding` gate reads this). */
export const TOURLESS_ROUTES: Record<string, string> = Object.fromEntries(
  PLACEHOLDER_ROUTES.map((route) => [
    route,
    'The shared route placeholder: the W4 task that rebuilds the screen brings its tour back.',
  ]),
)

/** The tour a route shows. */
export function tourIdOf(routeName: string): string {
  return routeName
}
