/** Palette search and pinned items for `pnpm dev`: a simplified copy of core `search`. */

import { txLegs, txMeta } from '@/shared/lib/transactions'
import { mockTransactions } from './demo'
import { mockCategories, mockContacts } from './ledger/seed'
import { alerts, budgets, goals, reminders } from './planning/state'
import { mockStore } from './tree'
import { mockEntityCard } from './relations'
import { mockSettings } from './vault-data'
import { workflows } from './workflows/state'

const PINNED_CAP = 30
const SEARCH_TX_CAP = 50

const entry = (kind: EntityKind, id: string, label: string, archived = false): SearchEntry => ({
  target: { kind, id },
  label,
  subject: null,
  archived,
})

export function mockSearchIndex(): SearchEntry[] {
  return [
    ...goals.map((goal) => entry('goal', goal.id, goal.name, !goal.active)),
    ...budgets.map((budget) => {
      const target = budget.target
      const subject: EntityRef | null =
        typeof target === 'object' && 'category' in target
          ? { kind: 'category', id: target.category }
          : typeof target === 'object' && 'store' in target && target.store.store_ids[0]
            ? { kind: 'store', id: target.store.store_ids[0] }
            : null
      return { ...entry('budget', budget.id, '', !budget.active), subject }
    }),
    ...mockCategories.map((category) => entry('category', category.key, category.name ?? '', category.archived)),
    ...mockContacts.map((contact) => entry('contact', contact.id, contact.name, contact.archived)),
    ...workflows.map((flow) => entry(flow.automation ? 'automation' : 'workflow', flow.id, flow.name, !flow.active)),
    ...reminders.filter((reminder) => !reminder.muted).map((rem) => entry('reminder', rem.id, rem.text, rem.done)),
    ...alerts.map((alert) => entry('alert', alert.id, alert.asset.symbol, !alert.active)),
  ]
}

export function mockSearchTransactions(query: string, categories: string[], limit: number): Transaction[] {
  const needle = query.trim().toLowerCase()
  const number = needle.replace(/\s/g, '').replace(',', '.')
  if (!needle && !categories.length) return []
  return mockTransactions
    .filter((tx) => {
      const meta = txMeta(tx)
      if (meta.categories.some((key) => categories.includes(key))) return true
      if (!needle) return false
      const names = [
        meta.comment,
        ...txLegs(tx).map((leg) => mockStore(leg.store_id)?.name ?? ''),
        ...meta.categories.map((key) => mockCategories.find((cat) => cat.key === key)?.name ?? ''),
      ]
      return (
        names.some((name) => name.toLowerCase().includes(needle)) ||
        (/\d/.test(number) && txLegs(tx).some((leg) => String(leg.amount).replace(/-/, '').startsWith(number)))
      )
    })
    .sort((left, right) => txMeta(right).timestamp.localeCompare(txMeta(left).timestamp))
    .slice(0, Math.min(Math.max(limit, 1), SEARCH_TX_CAP))
}

export function mockPinned(): PinnedView[] {
  return mockSettings.pinned.map((pin) => {
    if (pin.type === 'filter') return { pin, title: pin.name, value: null, currency: null, pct: null, missing: false }
    const card = mockEntityCard(pin.target)
    return { pin, title: card.title, value: card.value, currency: card.currency, pct: null, missing: card.missing }
  })
}

export function mockSetPinned(pins: Pinned[]): PinnedView[] {
  const kept: Pinned[] = []
  for (const pin of pins) {
    if (pin.type === 'filter' && !pin.name.trim())
      throw { code: 'INVALID_INPUT', message: 'a saved filter needs a name' }
    const clean: Pinned =
      pin.type === 'filter'
        ? { ...pin, id: pin.id || crypto.randomUUID(), name: pin.name.trim(), query: pin.query.replace(/^\?/, '') }
        : pin
    if (!kept.some((other) => JSON.stringify(other) === JSON.stringify(clean))) kept.push(clean)
  }
  mockSettings.pinned = kept.slice(0, PINNED_CAP)
  return mockPinned()
}
