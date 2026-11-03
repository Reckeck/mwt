import { type MaybeRefOrGetter, computed, toValue } from 'vue'

import { contactNets, isQuickContact } from '@/entities/contact'
import { useContactBalances, useContactSummaries, useContacts } from '@/shared/api/queries'

import type { ContactFilter, ContactRow, ContactScope, ContactTotals } from '../contact-table.types'

/** The contacts joined with their debts and last operation: every row the table can show. */
export function useContactRows() {
  const contacts = useContacts()
  const balances = useContactBalances()
  const summaries = useContactSummaries()

  const rows = computed<ContactRow[]>(() => {
    const balanceRows = balances.data.value ?? []
    const byContact = new Map((summaries.data.value ?? []).map((summary) => [summary.contact_id, summary]))
    return (contacts.data.value ?? []).map((contact) => {
      const summary = byContact.get(contact.id)
      const nets = contactNets(balanceRows, contact.id).sort((left, right) => Math.abs(right.net) - Math.abs(left.net))
      return {
        id: contact.id,
        contact,
        nets,
        debtBase: summary ? Number(summary.receivable_base) - Number(summary.payable_base) : 0,
        lastAt: summary?.last_tx_at ?? null,
        lastAmount: summary?.last_amount ?? null,
        lastAsset: summary?.last_asset ?? null,
        txCount: summary?.tx_count ?? 0,
        accountCount: contact.accounts.length,
      }
    })
  })

  const totals = computed<ContactTotals>(() => {
    const live = new Set(rows.value.filter((row) => !row.contact.archived).map((row) => row.id))
    const own = (summaries.data.value ?? []).filter((summary) => live.has(summary.contact_id))
    const receivable = own.filter((summary) => Number(summary.receivable_base) > Number(summary.payable_base))
    const payable = own.filter((summary) => Number(summary.payable_base) > Number(summary.receivable_base))
    return {
      receivable: own.reduce((sum, summary) => sum + Number(summary.receivable_base), 0),
      payable: own.reduce((sum, summary) => sum + Number(summary.payable_base), 0),
      receivableCount: receivable.length,
      payableCount: payable.length,
      missingRate: own.some((summary) => summary.missing_rate),
    }
  })

  const counts = computed<Record<ContactScope, number>>(() => {
    const live = rows.value.filter((row) => !row.contact.archived)
    return {
      all: live.length,
      debt: live.filter((row) => row.nets.length).length,
      quick: live.filter((row) => isQuickContact(row.contact)).length,
      archive: rows.value.length - live.length,
    }
  })

  /** Waits for the balances too, so the debt column does not fill in under the user's eyes. */
  const loading = computed(() => contacts.isPending.value || balances.isPending.value || summaries.isPending.value)
  const error = computed(() => contacts.error.value ?? balances.error.value ?? summaries.error.value)

  return { rows, totals, counts, loading, error, total: computed(() => contacts.data.value?.length ?? 0) }
}

/** The rows the chips, the tile direction, the kind and the search let through. */
export function filterRows(rows: ContactRow[], filter: MaybeRefOrGetter<ContactFilter>): ContactRow[] {
  const { search, scope, direction, kind } = toValue(filter)
  const needle = search.trim().toLowerCase()
  const compact = needle.replace(/\s/g, '')
  return rows.filter(({ contact, nets }) => {
    if (scope === 'archive' ? !contact.archived : contact.archived) return false
    if (scope === 'debt' && !nets.length) return false
    if (scope === 'quick' && !isQuickContact(contact)) return false
    if (direction === 'receivable' && !nets.some((net) => net.net > 0)) return false
    if (direction === 'payable' && !nets.some((net) => net.net < 0)) return false
    if (kind && contact.kind !== kind) return false
    if (!needle) return true
    return (
      contact.name.toLowerCase().includes(needle) ||
      contact.group.toLowerCase().includes(needle) ||
      contact.accounts.some((account) => account.value.replace(/\s/g, '').toLowerCase().includes(compact))
    )
  })
}
