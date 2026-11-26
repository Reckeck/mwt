import type { RouteRecordRaw } from 'vue-router'

const page = (name: string) => () => import('@/pages/automations').then((module) => module[name as keyof typeof module])

/** «Automations»: the user's own and their journal; schemes, reminders, alerts and sources behind «More». */
export const automationsRoutes: RouteRecordRaw[] = [
  {
    path: 'automations',
    component: page('AutomationsSection'),
    children: [
      {
        path: '',
        name: 'automations',
        component: page('AutomationsMine'),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'journal',
        name: 'automations-journal',
        component: page('AutomationsJournal'),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'workflows',
        name: 'automations-workflows',
        component: page('AutomationsWorkflows'),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'reminders',
        name: 'automations-reminders',
        component: page('AutomationsReminders'),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'alerts',
        name: 'automations-alerts',
        component: page('AutomationsAlerts'),
        meta: { i18n: 'nav.automations' },
      },
      {
        path: 'sources',
        name: 'automations-sources',
        component: page('AutomationsSources'),
        meta: { i18n: 'nav.automations' },
      },
    ],
  },
  {
    path: 'automations/workflows/new',
    name: 'workflow-new',
    component: page('WorkflowEdit'),
    meta: { i18n: 'nav.automations', fullscreen: true },
  },
  {
    path: 'automations/workflows/:id',
    name: 'workflow-edit',
    component: page('WorkflowEdit'),
    meta: { i18n: 'nav.automations', fullscreen: true },
  },
]
