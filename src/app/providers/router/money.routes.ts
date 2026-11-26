import type { RouteRecordRaw } from 'vue-router'

import { routePlaceholder } from './route-placeholder'

/**
 * The `money` address: the six Budget routes inside the `MoneySection` frame (its tabs are those children,
 * the same names as the budget section in `NAV_SECTIONS`), then the Capital and People screens that keep
 * their `money/…` addresses but sit outside the frame.
 */
export const moneyRoutes: RouteRecordRaw = {
  path: 'money',
  children: [
    {
      path: '',
      component: () => import('@/pages/money').then((page) => page.MoneySection),
      children: [
        {
          path: '',
          name: 'money',
          component: routePlaceholder,
          meta: { i18n: 'nav.money', group: 'spending' },
        },
        {
          path: 'month',
          name: 'money-month',
          component: () => import('@/pages/money').then((page) => page.MoneyMonth),
          meta: { i18n: 'nav.money', group: 'month' },
        },
        {
          path: 'plan',
          name: 'money-plan',
          component: () => import('@/pages/money').then((page) => page.MoneyPlan),
          meta: { i18n: 'nav.money', group: 'plan' },
        },
        {
          path: 'where',
          name: 'money-where',
          component: routePlaceholder,
          meta: { i18n: 'nav.money', group: 'where' },
        },
        {
          path: 'calculator',
          name: 'money-calculator',
          component: () => import('@/pages/money').then((page) => page.MoneyCalculator),
          meta: { i18n: 'nav.money', group: 'calculator' },
        },
        {
          path: 'health',
          name: 'money-health',
          component: routePlaceholder,
          meta: { i18n: 'nav.money', group: 'health' },
        },
      ],
    },
    {
      path: 'wealth',
      name: 'money-wealth',
      component: () => import('@/pages/money').then((page) => page.MoneyWealth),
      meta: { i18n: 'nav.money', group: 'wealth' },
    },
    {
      path: 'investments',
      name: 'money-investments',
      component: () => import('@/pages/money').then((page) => page.MoneyInvestments),
      meta: { i18n: 'nav.money', group: 'investments' },
    },
    {
      path: 'together',
      name: 'money-together',
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'together' },
    },
    {
      path: 'together/:id',
      name: 'money-together-event',
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'together', tab: 'money-together' },
    },
  ],
}
