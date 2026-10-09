import { createRouter, createWebHistory } from 'vue-router'

export const router = createRouter({
  history: createWebHistory(),
  routes: [
    {
      path: '/unlock',
      name: 'unlock',
      component: () => import('@/views/UnlockView.vue'),
      meta: { public: true },
    },
    {
      path: '/',
      component: () => import('@/views/AppShell.vue'),
      children: [
        {
          path: '',
          name: 'dashboard',
          component: () => import('@/views/DashboardView.vue'),
          meta: { i18n: 'nav.dashboard' },
        },
        {
          path: 'stores',
          name: 'stores',
          component: () => import('@/views/StoresView.vue'),
          meta: { i18n: 'nav.stores' },
        },
        {
          path: 'stores/:id',
          name: 'wallet',
          component: () => import('@/views/WalletView.vue'),
          meta: { i18n: 'nav.wallets' },
        },
        {
          path: 'transactions',
          name: 'transactions',
          component: () => import('@/views/TransactionsView.vue'),
          meta: { i18n: 'nav.transactions' },
        },
        {
          path: 'charts',
          name: 'charts',
          component: () => import('@/views/ChartsView.vue'),
          meta: { i18n: 'nav.charts' },
        },
        {
          path: 'trading',
          name: 'trading',
          component: () => import('@/views/TradingView.vue'),
          meta: { i18n: 'nav.trading' },
        },
        {
          path: 'settings',
          name: 'settings',
          component: () => import('@/views/SettingsView.vue'),
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
