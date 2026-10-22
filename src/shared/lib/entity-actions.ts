import { type Component, ref } from 'vue'
import type { RouteLocationRaw } from 'vue-router'
import {
  Bell,
  Calculator,
  CalendarClock,
  ExternalLink,
  Gauge,
  ListFilter,
  Pencil,
  Target,
  Workflow as WorkflowIcon,
} from 'lucide-vue-next'

import { emptyTxFilter, storeTxFilter, txFilterToQuery } from './tx-filter'

/** The dialogs a card can open with the entity filled in. */
export type EntityDialogKind = 'goal' | 'budget' | 'alert'

export interface EntityDialogRequest {
  dialog: EntityDialogKind
  target: EntityRef
  /** What the card shows as its name: an asset's ticker for a new alert. */
  title: string
}

export interface EntityAction {
  id: string
  icon: Component
  /** An i18n key under `entity.action.*`. */
  labelKey: string
  /** A navigation; a card closes behind it. */
  to?: (target: EntityRef) => RouteLocationRaw
  /** A dialog the shell opens over the card. */
  dialog?: EntityDialogKind
}

const transactions = (filter: TxFilter): RouteLocationRaw => ({ name: 'transactions', query: txFilterToQuery(filter) })

const operations = (filter: (id: string) => TxFilter): EntityAction => ({
  id: 'operations',
  icon: ListFilter,
  labelKey: 'entity.action.operations',
  to: (target) => transactions(filter(target.id)),
})

const page = (labelKey: string, to: (target: EntityRef) => RouteLocationRaw): EntityAction => ({
  id: 'page',
  icon: ExternalLink,
  labelKey,
  to,
})

/**
 * What can be done with each kind of entity, in one place: the card, a link's
 * menu and (later) the palette show the same list. «Create related» opens a
 * dialog with the entity already chosen.
 */
const ACTIONS: Record<EntityKind, EntityAction[]> = {
  store: [
    page('entity.action.openWallet', (target) => ({ name: 'wallet', params: { id: target.id } })),
    operations((id) => storeTxFilter(id)),
    { id: 'new-goal', icon: Target, labelKey: 'entity.action.newGoal', dialog: 'goal' },
  ],
  tx: [],
  category: [
    operations((id) => emptyTxFilter({ category: id })),
    { id: 'new-budget', icon: Gauge, labelKey: 'entity.action.newBudget', dialog: 'budget' },
    page('entity.action.openBudgets', () => ({ name: 'money-plan' })),
  ],
  contact: [
    operations((id) => emptyTxFilter({ contact_id: id })),
    page('entity.action.openContacts', () => ({ name: 'transactions-contacts' })),
  ],
  goal: [page('entity.action.openGoals', () => ({ name: 'money-plan' }))],
  scenario: [
    {
      ...page('entity.action.openCalculator', (target) => ({
        name: 'money-calculator',
        query: { scenario: target.id },
      })),
      icon: Calculator,
    },
  ],
  budget: [page('entity.action.openBudgets', () => ({ name: 'money-plan' }))],
  automation: [
    {
      id: 'edit',
      icon: Pencil,
      labelKey: 'entity.action.edit',
      to: (target) => ({ name: 'automations', query: { edit: target.id } }),
    },
  ],
  workflow: [
    {
      id: 'editor',
      icon: WorkflowIcon,
      labelKey: 'entity.action.openEditor',
      to: (target) => ({ name: 'workflow-edit', params: { id: target.id } }),
    },
  ],
  reminder: [
    { ...page('entity.action.openReminders', () => ({ name: 'automations-reminders' })), icon: CalendarClock },
  ],
  alert: [page('entity.action.openAlerts', () => ({ name: 'automations-alerts' }))],
  asset: [
    { id: 'new-alert', icon: Bell, labelKey: 'entity.action.newAlert', dialog: 'alert' },
    page('entity.action.openAlerts', () => ({ name: 'automations-alerts' })),
  ],
}

export function entityActions(kind: EntityKind): EntityAction[] {
  return ACTIONS[kind] ?? []
}

/** The dialog the shell should open; one at a time, over the cards. */
export const entityDialog = ref<EntityDialogRequest | null>(null)

export function requestEntityDialog(request: EntityDialogRequest) {
  entityDialog.value = request
}
