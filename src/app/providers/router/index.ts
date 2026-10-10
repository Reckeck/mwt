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
          name: 'transactions',
          component: () => import('@/pages/transactions').then((page) => page.Transactions),
          meta: { i18n: 'nav.transactions' },
        },
        {
          path: 'charts',
          name: 'charts',
          component: () => import('@/pages/charts').then((page) => page.Charts),
          meta: { i18n: 'nav.charts' },
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
