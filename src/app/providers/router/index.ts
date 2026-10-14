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
            { path: 'losses', redirect: { name: 'analytics-losses' } },
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
            // Rates moved to Settings → Money; the contacts stay here with the window open over them.
            { path: 'rates', redirect: { name: 'directory-contacts', query: { settings: 'rates' } } },
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
              path: 'vars',
              name: 'workflows-vars',
              component: () => import('@/pages/workflows').then((page) => page.WorkflowsVars),
              meta: { i18n: 'nav.workflows' },
            },
            {
              path: 'queue',
              name: 'workflows-queue',
              component: () => import('@/pages/workflows').then((page) => page.WorkflowsQueue),
              meta: { i18n: 'nav.workflows' },
            },
            {
              path: 'runs',
              name: 'workflows-runs',
              component: () => import('@/pages/workflows').then((page) => page.WorkflowsRuns),
              meta: { i18n: 'nav.workflows' },
            },
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
