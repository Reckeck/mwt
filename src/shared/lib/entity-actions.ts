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
  Search,
  Target,
  Workflow as WorkflowIcon,
} from 'lucide-vue-next'

import { assetSearchText } from './asset-ref'
import { hereBackQuery } from './drill'
import { entityPage } from './entity-link'
import { similarLocation, txId, txStoreId } from './transactions'
import { txDetailsTx } from './tx-details'
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
  /** True when the action makes no sense for this entity (the menu leaves it out). */
  hidden?: (target: EntityRef) => boolean
}

/** The list of operations; it offers «← <this screen>» back to where the card was opened. */
const transactions = (filter: TxFilter): RouteLocationRaw => ({
  name: 'transactions',
  query: { ...txFilterToQuery(filter), ...hereBackQuery() },
})

const operations = (filter: (id: string) => TxFilter, labelKey = 'entity.action.operations'): EntityAction => ({
  id: 'operations',
  icon: ListFilter,
  labelKey,
  to: (target) => transactions(filter(target.id)),
})

const page = (labelKey: string, to: (target: EntityRef) => RouteLocationRaw): EntityAction => ({
  id: 'page',
  icon: ExternalLink,
  labelKey,
  to,
})

/** `?focus=` lights the entity's row in the list it leads to (`useFocusRow`). */
const focusPage = (labelKey: string, name: string, icon?: Component): EntityAction => ({
  ...page(labelKey, (target) => ({ name, query: { focus: target.id } })),
  ...(icon ? { icon } : {}),
})

/** The operation the card cache holds, if it is this one: a link's menu has only the id. */
const cachedTx = (target: EntityRef): Transaction | null => {
  const tx = txDetailsTx.value
  return tx && txId(tx) === target.id ? tx : null
}

/** Category in «Траты» for the month: the same jump as Ctrl K (Paths 2 · 8). */
export function spendingOfCategory(category: string, month = new Date().toISOString().slice(0, 7)): RouteLocationRaw {
  return { name: 'money', query: { from: month, to: month, category } }
}

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
  tx: [
    {
      id: 'open-wallet',
      icon: ExternalLink,
      labelKey: 'entity.action.openWallet',
      to: (target) =>
        entityPage({ kind: 'store', id: txStoreId(cachedTx(target) as Transaction) }) ?? { name: 'stores' },
      hidden: (target) => !cachedTx(target),
    },
    {
      id: 'similar',
      icon: Search,
      labelKey: 'tx.menu.similar',
      to: (target) => similarLocation(cachedTx(target) as Transaction) ?? { name: 'transactions' },
      hidden: (target) => {
        const tx = cachedTx(target)
        return !tx || !similarLocation(tx)
      },
    },
  ],
  category: [
    operations((id) => emptyTxFilter({ category: id })),
    page('entity.action.openSpending', (target) => spendingOfCategory(target.id)),
    { id: 'new-budget', icon: Gauge, labelKey: 'entity.action.newBudget', dialog: 'budget' },
    page('entity.action.openBudgets', () => ({ name: 'money-plan' })),
  ],
  contact: [
    operations((id) => emptyTxFilter({ contact_id: id })),
    page('entity.action.openContact', (target) => ({ name: 'transactions-contacts', query: { contact: target.id } })),
  ],
  goal: [focusPage('entity.action.openGoals', 'money-plan')],
  scenario: [
    {
      ...page('entity.action.openCalculator', (target) => ({
        name: 'money-calculator',
        query: { scenario: target.id },
      })),
      icon: Calculator,
    },
  ],
  budget: [focusPage('entity.action.openBudgets', 'money-plan')],
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
  reminder: [focusPage('entity.action.openReminders', 'automations-reminders', CalendarClock)],
  alert: [focusPage('entity.action.openAlerts', 'automations-alerts')],
  asset: [
    operations((id) => emptyTxFilter({ text: assetSearchText(id) }), 'entity.action.assetOperations'),
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
