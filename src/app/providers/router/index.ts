import { watch } from 'vue'
import { type RouteLocationNormalized, createRouter, createWebHistory } from 'vue-router'

import { i18n } from '@/shared/config/i18n'

import { automationsRoutes } from './automations.routes'
import { legacyRoutes } from './legacy.routes'
import { moneyRoutes } from './money.routes'

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
            // Losses live in Money now; old links and tours land there.
            { path: 'losses', redirect: { name: 'money-investments' } },
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
            {
              path: 'bonuses',
              name: 'transactions-bonuses',
              component: () => import('@/pages/transactions').then((page) => page.TransactionsBonuses),
              meta: { i18n: 'nav.transactions' },
            },
          ],
        },
        moneyRoutes,
        ...automationsRoutes,
        ...legacyRoutes,
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
