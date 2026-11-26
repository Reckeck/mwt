import { watch } from 'vue'
import { type RouteLocationNormalized, createRouter, createWebHistory } from 'vue-router'

import { i18n } from '@/shared/config/i18n'

import { automationsRoutes } from './automations.routes'
import { cryptoRoutes } from './crypto.routes'
import { moneyRoutes } from './money.routes'
import { routePlaceholder } from './route-placeholder'

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
          component: routePlaceholder,
          meta: { i18n: 'nav.dashboard' },
        },
        {
          path: 'stores',
          name: 'stores',
          component: () => import('@/pages/stores').then((page) => page.StoresPage),
          meta: { i18n: 'nav.stores' },
        },
        {
          path: 'stores/:id',
          name: 'wallet',
          component: routePlaceholder,
          meta: { i18n: 'nav.wallets' },
        },
        {
          path: 'stores/:id/settings',
          name: 'wallet-settings',
          component: routePlaceholder,
          meta: { i18n: 'walletSettings.nav' },
        },
        {
          path: 'transactions',
          children: [
            {
              path: '',
              name: 'transactions',
              component: routePlaceholder,
              meta: { i18n: 'nav.transactions' },
            },
            {
              // Contacts left the Transactions section; old links keep their query.
              path: 'contacts',
              redirect: (to) => ({ name: 'contacts', query: to.query, hash: to.hash }),
            },
            {
              // What the crypto module read from its sources; the filter is in the query (spec E10).
              path: 'crypto',
              name: 'crypto-operations',
              component: routePlaceholder,
              meta: { i18n: 'cryptoTrack.operations.title' },
            },
            {
              path: 'calendar',
              name: 'transactions-calendar',
              component: routePlaceholder,
              meta: { i18n: 'nav.transactions' },
            },
            {
              path: 'bonuses',
              name: 'transactions-bonuses',
              component: routePlaceholder,
              meta: { i18n: 'nav.transactions' },
            },
          ],
        },
        {
          path: 'contacts',
          name: 'contacts',
          component: routePlaceholder,
          meta: { i18n: 'nav.contacts' },
        },
        moneyRoutes,
        cryptoRoutes,
        ...automationsRoutes,
        {
          // The settings window opens over any page; old links land on the dashboard with it open.
          path: 'settings',
          name: 'settings',
          redirect: { name: 'dashboard', query: { settings: 'list' } },
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
