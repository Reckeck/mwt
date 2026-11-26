import type { RouteRecordRaw } from 'vue-router'

import { routePlaceholder } from './route-placeholder'

/** «Automations»: the user's own and their journal; schemes, reminders, alerts and sources behind «More». */
export const automationsRoutes: RouteRecordRaw[] = [
  {
    path: 'automations',
    children: [
      {
        path: '',
        name: 'automations',
        component: routePlaceholder,
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'journal',
        name: 'automations-journal',
        component: routePlaceholder,
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'workflows',
        name: 'automations-workflows',
        component: routePlaceholder,
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'reminders',
        name: 'automations-reminders',
        component: routePlaceholder,
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'alerts',
        name: 'automations-alerts',
        component: routePlaceholder,
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'sources',
        name: 'automations-sources',
        component: routePlaceholder,
        meta: { i18n: 'nav.automations' },
      },
    ],
  },
  {
    path: 'automations/workflows/new',
    name: 'workflow-new',
    component: routePlaceholder,
    meta: { i18n: 'nav.automations', fullscreen: true },
  },
  {
    path: 'automations/workflows/:id',
    name: 'workflow-edit',
    component: routePlaceholder,
    meta: { i18n: 'nav.automations', fullscreen: true },
  },
]
