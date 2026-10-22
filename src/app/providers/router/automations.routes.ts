import type { RouteRecordRaw } from 'vue-router'

const page = () => import('@/pages/automations')

/** «Automations»: the user's own and their journal; schemes, reminders, alerts and sources behind «More». */
export const automationsRoutes: RouteRecordRaw[] = [
  {
    path: 'automations',
    component: () => page().then((module) => module.AutomationsSection),
    children: [
      {
        path: '',
        name: 'automations',
        component: () => page().then((module) => module.AutomationsMine),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'journal',
        name: 'automations-journal',
        component: () => page().then((module) => module.AutomationsJournal),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'workflows',
        name: 'automations-workflows',
        component: () => page().then((module) => module.AutomationsWorkflows),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'reminders',
        name: 'automations-reminders',
        component: () => page().then((module) => module.AutomationsReminders),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'alerts',
        name: 'automations-alerts',
        component: () => page().then((module) => module.AutomationsAlerts),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'sources',
        name: 'automations-sources',
        component: () => page().then((module) => module.AutomationsSources),
        meta: { i18n: 'nav.automations' },
      },
    ],
  },
  {
    path: 'automations/workflows/new',
    name: 'workflow-new',
    component: () => page().then((module) => module.WorkflowEdit),
    meta: { i18n: 'nav.automations', fullscreen: true },
  },
  {
    path: 'automations/workflows/:id',
    name: 'workflow-edit',
    component: () => page().then((module) => module.WorkflowEdit),
    meta: { i18n: 'nav.automations', fullscreen: true },
  },
]
