import type { RouteLocationGeneric, RouteRecordRaw } from 'vue-router'

/** Same query, another screen. */
const keep =
  (name: string, extra: Record<string, string> = {}) =>
  (to: RouteLocationGeneric) => ({
    name,
    query: { ...to.query, ...extra },
  })

/**
 * Paths of the interface before the redesign. Pins, saved filters, notices and
 * links from other apps point here; each lands on its new place.
 */
export const legacyRoutes: RouteRecordRaw[] = [
  { path: 'charts', redirect: { name: 'money-investments' } },
  { path: 'analytics', redirect: keep('money') },
  { path: 'analytics/flow', redirect: keep('money-where') },
  { path: 'analytics/wealth', redirect: keep('money-wealth') },
  { path: 'analytics/investments', redirect: keep('money-investments') },
  { path: 'analytics/saldo', redirect: keep('money-month') },
  { path: 'analytics/health', redirect: keep('money-health') },
  // The nine reports of before, folded into the sections.
  { path: 'analytics/cashflow', redirect: keep('money-where') },
  { path: 'analytics/year', redirect: keep('money-where', { view: 'year' }) },
  { path: 'analytics/categories', redirect: keep('money-where', { view: 'categories' }) },
  { path: 'analytics/networth', redirect: keep('money-wealth') },
  { path: 'analytics/stores', redirect: keep('money-wealth') },
  { path: 'analytics/inflation', redirect: keep('money-wealth', { real: '1' }) },
  { path: 'analytics/pnl', redirect: keep('money-investments') },
  { path: 'analytics/portfolio', redirect: keep('money-investments') },
  { path: 'analytics/losses', redirect: keep('money-investments') },
  { path: 'planning', redirect: { name: 'automations' } },
  { path: 'planning/automations', redirect: { name: 'automations' } },
  { path: 'planning/budgets', redirect: { name: 'money-plan' } },
  { path: 'planning/goals', redirect: { name: 'money-plan' } },
  { path: 'planning/calculator', redirect: { name: 'money-calculator' } },
  { path: 'planning/sources', redirect: { name: 'automations-sources' } },
  { path: 'planning/reminders', redirect: { name: 'automations-reminders' } },
  { path: 'planning/alerts', redirect: { name: 'automations-alerts' } },
  { path: 'workflows', redirect: keep('automations-workflows') },
  { path: 'workflows/journal', redirect: { name: 'automations-journal' } },
  // The queue and the runs are one journal; variables open as a panel.
  { path: 'workflows/queue', redirect: { name: 'automations-journal' } },
  { path: 'workflows/runs', redirect: { name: 'automations-journal' } },
  { path: 'workflows/vars', redirect: { name: 'automations-workflows', query: { vars: '1' } } },
  { path: 'workflows/new', redirect: { name: 'workflow-new' } },
  { path: 'workflows/:id', redirect: (to) => ({ name: 'workflow-edit', params: { id: to.params.id } }) },
  // Contacts live under Transactions; rates in Settings → Money.
  { path: 'directory', redirect: { name: 'transactions-contacts' } },
  { path: 'directory/contacts', redirect: { name: 'transactions-contacts' } },
  { path: 'directory/rates', redirect: { name: 'transactions-contacts', query: { settings: 'rates' } } },
]
