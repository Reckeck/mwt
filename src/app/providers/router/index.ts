import { watch } from 'vue'
import { type RouteLocationNormalized, createRouter, createWebHistory } from 'vue-router'

import { i18n } from '@/shared/config/i18n'

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/unlock',
      name: 'unlock',
      component: () => import('@/pages/unlock').then((page) => page.Unlock),
      meta: { public: true },
    },
    {
      path: '/',
      component: () => import('@/pages/app-shell').then((page) => page.AppShell),
      children: [
        {
          path: '',
          name: 'dashboard',
          component: () => import('@/pages/dashboard').then((page) => page.Dashboard),
          meta: { i18n: 'nav.dashboard' },
        },
        {
          path: 'stores',
          name: 'stores',
          component: () => import('@/pages/stores').then((page) => page.Stores),
          meta: { i18n: 'nav.stores' },
        },
        {
          path: 'stores/:id',
          name: 'wallet',
          component: () => import('@/pages/wallet').then((page) => page.Wallet),
          meta: { i18n: 'nav.wallets' },
        },
        {
          path: 'stores/:id/settings',
          name: 'wallet-settings',
          component: () => import('@/pages/wallet-settings').then((page) => page.WalletSettings),
          meta: { i18n: 'walletSettings.nav' },
        },
        {
          path: 'transactions',
          component: () => import('@/pages/transactions').then((page) => page.TransactionsSection),
          children: [
            {
              path: '',
              name: 'transactions',
              component: () => import('@/pages/transactions').then((page) => page.Transactions),
              meta: { i18n: 'nav.transactions' },
            },
            // Losses live in Analytics now; old links and tours land there.
            { path: 'losses', redirect: { name: 'analytics-investments' } },
            // The money graph was dropped (2026-10): old links open the list.
            { path: 'graph', redirect: (to) => ({ name: 'transactions', query: to.query }) },
            {
              path: 'contacts',
              name: 'transactions-contacts',
              component: () => import('@/pages/transactions').then((page) => page.TransactionsContacts),
              meta: { i18n: 'nav.transactions' },
            },
            {
              path: 'calendar',
              name: 'transactions-calendar',
              component: () => import('@/pages/transactions').then((page) => page.TransactionsCalendar),
              meta: { i18n: 'nav.transactions' },
            },
          ],
        },
        { path: 'charts', redirect: { name: 'analytics-investments' } },
        {
          path: 'analytics',
          component: () => import('@/pages/analytics').then((page) => page.Analytics),
          children: [
            {
              path: '',
              name: 'analytics',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsOverview),
              meta: { i18n: 'nav.analytics', group: 'overview' },
            },
            {
              path: 'flow',
              name: 'analytics-flow',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsFlow),
              meta: { i18n: 'nav.analytics', group: 'flow' },
            },
            {
              path: 'wealth',
              name: 'analytics-wealth',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsWealth),
              meta: { i18n: 'nav.analytics', group: 'wealth' },
            },
            {
              path: 'investments',
              name: 'analytics-investments',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsInvestments),
              meta: { i18n: 'nav.analytics', group: 'investments' },
            },
            // The nine reports of before, folded into the sections; old links keep working.
            { path: 'cashflow', redirect: (to) => ({ name: 'analytics-flow', query: to.query }) },
            { path: 'year', redirect: (to) => ({ name: 'analytics-flow', query: { ...to.query, view: 'year' } }) },
            {
              path: 'categories',
              redirect: (to) => ({ name: 'analytics-flow', query: { ...to.query, view: 'categories' } }),
            },
            { path: 'networth', redirect: (to) => ({ name: 'analytics-wealth', query: to.query }) },
            { path: 'stores', redirect: (to) => ({ name: 'analytics-wealth', query: to.query }) },
            { path: 'inflation', redirect: (to) => ({ name: 'analytics-wealth', query: { ...to.query, real: '1' } }) },
            { path: 'pnl', redirect: (to) => ({ name: 'analytics-investments', query: to.query }) },
            { path: 'portfolio', redirect: (to) => ({ name: 'analytics-investments', query: to.query }) },
            { path: 'losses', redirect: (to) => ({ name: 'analytics-investments', query: to.query }) },
          ],
        },
        // Contacts live under Transactions now; old links land there.
        { path: 'directory', redirect: { name: 'transactions-contacts' } },
        { path: 'directory/contacts', redirect: { name: 'transactions-contacts' } },
        // Rates moved to Settings → Money; the contacts stay under the window open over them.
        { path: 'directory/rates', redirect: { name: 'transactions-contacts', query: { settings: 'rates' } } },
        {
          path: 'planning',
          component: () => import('@/pages/planning').then((page) => page.Planning),
          children: [
            { path: '', redirect: { name: 'planning-automations' } },
            {
              path: 'automations',
              name: 'planning-automations',
              component: () => import('@/pages/planning').then((page) => page.PlanningAutomations),
              meta: { i18n: 'nav.planning' },
            },
            {
              path: 'budgets',
              name: 'planning-budgets',
              component: () => import('@/pages/planning').then((page) => page.PlanningBudgets),
              meta: { i18n: 'nav.planning' },
            },
            {
              path: 'goals',
              name: 'planning-goals',
              component: () => import('@/pages/planning').then((page) => page.PlanningGoals),
              meta: { i18n: 'nav.planning' },
            },
            {
              path: 'reminders',
              name: 'planning-reminders',
              component: () => import('@/pages/planning').then((page) => page.PlanningReminders),
              meta: { i18n: 'nav.planning' },
            },
            {
              path: 'alerts',
              name: 'planning-alerts',
              component: () => import('@/pages/planning').then((page) => page.PlanningAlerts),
              meta: { i18n: 'nav.planning' },
            },
          ],
        },
        {
          path: 'workflows',
          component: () => import('@/pages/workflows').then((page) => page.Workflows),
          children: [
            {
              path: '',
              name: 'workflows',
              component: () => import('@/pages/workflows').then((page) => page.WorkflowsList),
              meta: { i18n: 'nav.workflows' },
            },
            {
              path: 'journal',
              name: 'workflows-journal',
              component: () => import('@/pages/workflows').then((page) => page.WorkflowsJournal),
              meta: { i18n: 'nav.workflows' },
            },
            // The queue and the runs are one journal now; variables open as a panel.
            { path: 'queue', redirect: { name: 'workflows-journal' } },
            { path: 'runs', redirect: { name: 'workflows-journal' } },
            { path: 'vars', redirect: { name: 'workflows', query: { vars: '1' } } },
          ],
        },
        {
          path: 'workflows/new',
          name: 'workflow-new',
          component: () => import('@/pages/workflows').then((page) => page.WorkflowEdit),
          meta: { i18n: 'nav.workflows' },
        },
        {
          path: 'workflows/:id',
          name: 'workflow-edit',
          component: () => import('@/pages/workflows').then((page) => page.WorkflowEdit),
          meta: { i18n: 'nav.workflows' },
        },
        {
          // The settings window opens over any page; old links land on the dashboard with it open.
          path: 'settings',
          name: 'settings',
          redirect: { name: 'dashboard', query: { settings: 'general' } },
        },
      ],
    },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
})

/** The window title: the screen's name in the UI language, then the app's. */
function setTitle(route: RouteLocationNormalized) {
  const app = i18n.global.t('app.title')
  const key = route.meta.i18n as string | undefined
  document.title = key ? `${i18n.global.t(key)} · ${app}` : app
}

router.afterEach(setTitle)
watch(i18n.global.locale, () => setTitle(router.currentRoute.value))
