import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Building2, CreditCard, Hash, Landmark, Link, PenLine, Phone, User, Wallet } from 'lucide-vue-next'

import { call } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

import type { AccountOption, AccountRefPickerProps } from './account-ref-picker.types'

export const RAW = 'raw'
/** The «not set» row. Reka throws on an item whose value is `''`, which left the list half-rendered. */
export const NONE = 'none'

/** A side as the list's value: `store:<id>`, `contact:<id>[:<account>]`, `raw` or `none`. */
export function encode(value: AccountRef | null): string {
  if (!value) return NONE
  if ('store' in value) return `store:${value.store}`
  if ('contact' in value) {
    const { contact_id: contactId, account_id: accountId } = value.contact
    return accountId ? `contact:${contactId}:${accountId}` : `contact:${contactId}`
  }
  return RAW
}

const DETAIL_ICONS: Record<DetailKind, typeof User> = {
  card: CreditCard,
  crypto_address: Wallet,
  iban: Landmark,
  account: Landmark,
  phone: Phone,
  url: Link,
  other: Hash,
}

export function iconOf(row: AccountOption) {
  if (row.detailKind) return DETAIL_ICONS[row.detailKind]
  return row.value.startsWith('store:') ? Building2 : row.value === RAW ? PenLine : User
}

/**
 * Every side the picker offers, once each: the wallets (path and balance under
 * the name), then each contact followed by its active accounts, then free text.
 */
export function useAccountOptions(
  props: Required<Pick<AccountRefPickerProps, 'stores' | 'contacts' | 'raw'>> & { excludeStoreId: string | null },
) {
  const { t } = useI18n()
  const settings = useSettingsStore()

  const tree = useQuery({
    queryKey: STORE_TREE_QUERY_KEY,
    queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
    enabled: computed(() => props.stores),
  })
  const contactList = useQuery({
    queryKey: ['contacts'],
    queryFn: () => call<Contact[]>('list_contacts'),
    enabled: computed(() => props.contacts),
  })

  const storeOptions = computed<AccountOption[]>(() =>
    flattenStoreTree(tree.data.value ?? [])
      .filter((node) => !node.store.archived && node.store.id !== props.excludeStoreId)
      .map((node) => {
        const parent = node.path.slice(0, -1).join(' › ')
        const binding =
          typeof node.store.binding === 'object' && 'asset' in node.store.binding ? node.store.binding.asset.symbol : ''
        const balance = formatMoney(node.subtree_base, { currency: settings.baseCurrency })
        const detail = [parent || t(`stores.kind.${node.store.kind}`), binding, balance].filter(Boolean).join(' · ')
        return {
          value: `store:${node.store.id}`,
          title: node.store.name,
          detail,
          group: t('tx.side.myWallets'),
          haystack: `${node.path.join(' ')} ${binding}`.toLowerCase(),
        }
      }),
  )

  /** A contact, then each of its active accounts: the card number is searchable. */
  const contactOptions = computed<AccountOption[]>(() =>
    (contactList.data.value ?? [])
      .filter((contact) => !contact.archived)
      .sort((left, right) => left.group.localeCompare(right.group) || left.name.localeCompare(right.name))
      .flatMap((contact) => {
        const group = contact.group.trim() || t('tx.side.contacts')
        const accounts = contact.accounts.filter((account) => account.active)
        return [
          {
            value: `contact:${contact.id}`,
            title: contact.name,
            detail: accounts.length
              ? t('tx.side.contactAccounts', { count: accounts.length })
              : t(`contactKind.${contact.kind}`),
            group,
            haystack: `${contact.name} ${contact.group}`.toLowerCase(),
          },
          ...accounts.map((account) => ({
            value: `contact:${contact.id}:${account.id}`,
            title: account.label || t(`detailKind.${account.kind}`),
            detail: [account.value, account.currency?.toUpperCase() ?? ''].filter(Boolean).join(' · '),
            group,
            haystack: `${contact.name} ${account.label} ${account.value} ${account.currency ?? ''}`.toLowerCase(),
            parent: `contact:${contact.id}`,
            detailKind: account.kind,
          })),
        ]
      }),
  )

  return computed<AccountOption[]>(() => {
    const rows = [...(props.stores ? storeOptions.value : []), ...(props.contacts ? contactOptions.value : [])]
    if (props.raw) {
      rows.push({ value: RAW, title: t('tx.side.other'), detail: t('tx.side.otherHint'), group: '', haystack: '' })
    }
    // One row per side: the same wallet or account never shows twice.
    const seen = new Set<string>()
    return rows.filter((row) => !seen.has(row.value) && seen.add(row.value))
  })
}
