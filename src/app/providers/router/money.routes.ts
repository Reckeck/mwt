import type { RouteRecordRaw } from 'vue-router'

const page = () => import('@/pages/money')

/** «Money»: spending, where it went and the plan, then five behind «More». */
export const moneyRoutes: RouteRecordRaw = {
  path: 'money',
  component: () => page().then((module) => module.MoneySection),
  children: [
    {
      path: '',
      name: 'money',
      component: () => page().then((module) => module.MoneySpending),
      meta: { i18n: 'nav.money', group: 'spending' },
    },
    {
      path: 'where',
      name: 'money-where',
      component: () => page().then((module) => module.MoneyWhere),
      meta: { i18n: 'nav.money', group: 'where' },
    },
    {
      path: 'plan',
      name: 'money-plan',
      component: () => page().then((module) => module.MoneyPlan),
      meta: { i18n: 'nav.money', group: 'plan' },
    },
    {
      path: 'wealth',
      name: 'money-wealth',
      component: () => page().then((module) => module.MoneyWealth),
      meta: { i18n: 'nav.money', group: 'wealth' },
    },
    {
      path: 'investments',
      name: 'money-investments',
      component: () => page().then((module) => module.MoneyInvestments),
      meta: { i18n: 'nav.money', group: 'investments' },
    },
    {
      path: 'month',
      name: 'money-month',
      component: () => page().then((module) => module.MoneyMonth),
      meta: { i18n: 'nav.money', group: 'month' },
    },
    {
      path: 'health',
      name: 'money-health',
      component: () => page().then((module) => module.MoneyHealth),
      meta: { i18n: 'nav.money', group: 'health' },
    },
    {
      path: 'calculator',
      name: 'money-calculator',
      component: () => page().then((module) => module.MoneyCalculator),
      meta: { i18n: 'nav.money', group: 'calculator' },
    },
    {
      path: 'together/:id',
      name: 'money-together-event',
      component: () => import('@/pages/money-together-event').then((module) => module.MoneyTogetherEvent),
      meta: { i18n: 'nav.money', group: 'plan' },
    },
  ],
}
