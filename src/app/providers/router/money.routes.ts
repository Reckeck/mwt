import type { RouteRecordRaw } from 'vue-router'

import { routePlaceholder } from './route-placeholder'

/** «Money»: spending, where it went, the plan and the shared events, then five behind «More». */
export const moneyRoutes: RouteRecordRaw = {
  path: 'money',
  children: [
    {
      path: '',
      name: 'money',
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'spending' },
    },
    {
      path: 'where',
      name: 'money-where',
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'where' },
    },
    {
      path: 'plan',
      name: 'money-plan',
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'plan' },
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
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'investments' },
    },
    {
      path: 'month',
      name: 'money-month',
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'month' },
    },
    {
      path: 'health',
      name: 'money-health',
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'health' },
    },
    {
      path: 'calculator',
      name: 'money-calculator',
      component: routePlaceholder,
      meta: { i18n: 'nav.money', group: 'calculator' },
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
