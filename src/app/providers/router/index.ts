import { createRouter, createWebHistory } from 'vue-router'

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
          path: 'transactions',
          component: () => import('@/pages/transactions').then((page) => page.TransactionsSection),
          children: [
            {
              path: '',
              name: 'transactions',
              component: () => import('@/pages/transactions').then((page) => page.Transactions),
              meta: { i18n: 'nav.transactions' },
            },
            {
              path: 'losses',
              name: 'transactions-losses',
              component: () => import('@/pages/transactions').then((page) => page.TransactionsLosses),
              meta: { i18n: 'nav.transactions' },
            },
            {
              path: 'graph',
              name: 'transactions-graph',
              component: () => import('@/pages/transactions').then((page) => page.TransactionsGraph),
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
        { path: 'charts', redirect: { name: 'analytics-portfolio' } },
        {
          path: 'analytics',
          component: () => import('@/pages/analytics').then((page) => page.Analytics),
          children: [
            { path: '', redirect: { name: 'analytics-cashflow' } },
            {
              path: 'cashflow',
              name: 'analytics-cashflow',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'cashflow' },
            },
            {
              path: 'year',
              name: 'analytics-year',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'year' },
            },
            {
              path: 'categories',
              name: 'analytics-categories',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'categories' },
            },
            {
              path: 'networth',
              name: 'analytics-networth',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'networth' },
            },
            {
              path: 'stores',
              name: 'analytics-stores',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'stores' },
            },
            {
              path: 'pnl',
              name: 'analytics-pnl',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'pnl' },
            },
            {
              path: 'losses',
              name: 'analytics-losses',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'losses' },
            },
            {
              path: 'portfolio',
              name: 'analytics-portfolio',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'portfolio' },
            },
            {
              path: 'inflation',
              name: 'analytics-inflation',
              component: () => import('@/pages/analytics').then((page) => page.AnalyticsTab),
              meta: { i18n: 'nav.analytics', report: 'inflation' },
            },
          ],
        },
        {
          path: 'directory',
          component: () => import('@/pages/directory').then((page) => page.Directory),
          children: [
            { path: '', redirect: { name: 'directory-contacts' } },
            {
              path: 'contacts',
              name: 'directory-contacts',
              component: () => import('@/pages/directory').then((page) => page.DirectoryContacts),
              meta: { i18n: 'nav.directory' },
            },
            {
              path: 'rates',
              name: 'directory-rates',
              component: () => import('@/pages/directory').then((page) => page.DirectoryRates),
              meta: { i18n: 'nav.directory' },
            },
          ],
        },
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
              path: 'utilities',
              name: 'planning-utilities',
              component: () => import('@/pages/planning').then((page) => page.PlanningUtilities),
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
          path: 'settings',
          name: 'settings',
          component: () => import('@/pages/settings').then((page) => page.Settings),
          meta: { i18n: 'nav.settings' },
        },
      ],
    },
    { path: '/:pathMatch(.*)*', redirect: '/' },
  ],
})

router.afterEach((to) => {
  const key = to.meta.i18n as string | undefined
  document.title = key ? `TradeApp · ${key}` : 'TradeApp'
})
