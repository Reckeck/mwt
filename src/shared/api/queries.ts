import { type MaybeRefOrGetter, toRef } from 'vue'
import { useQuery } from '@tanstack/vue-query'

import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'

import { call } from './ipc'

/**
 * The lists half the screens read: one definition each, so every screen shares
 * one cache entry with the same key and the same fetch (and a change to how it
 * is loaded happens in one place). `enabled` waits for the dialog or the field
 * that needs the list.
 */
export const useStoreTree = (enabled: MaybeRefOrGetter<boolean> = true) =>
  useQuery({
    queryKey: STORE_TREE_QUERY_KEY,
    queryFn: () => call<StoreTreeNode[]>('get_store_tree'),
    enabled: toRef(enabled),
  })

export const useContacts = (enabled: MaybeRefOrGetter<boolean> = true) =>
  useQuery({ queryKey: ['contacts'], queryFn: () => call<Contact[]>('list_contacts'), enabled: toRef(enabled) })

export const useCategories = (enabled: MaybeRefOrGetter<boolean> = true) =>
  useQuery({ queryKey: ['categories'], queryFn: () => call<Category[]>('list_categories'), enabled: toRef(enabled) })

/** Which operations belong to which property (`tx_property_links`); every saved operation refreshes it. */
export const TX_PROPERTY_LINKS_KEY = ['tx-property-links']

/** The emergency fund (`emergency_fund_get`): the wallet switch and the checkup both change it. */
export const EMERGENCY_FUND_KEY = ['emergency-fund']
/** The capital checkup (`capital_health`): every change to the fund or its settings refreshes it. */
export const CAPITAL_HEALTH_KEY = ['capital-health']

/** The fund status, shared by the wallet switch and the checkup. */
export const useEmergencyStatus = () =>
  useQuery({ queryKey: EMERGENCY_FUND_KEY, queryFn: () => call<EmergencyStatus>('emergency_fund_get') })

/** The baskets and hand-placed assets (`asset_classes_get`). */
export const ASSET_CLASSES_KEY = ['asset-classes']
/** Where the month's free money goes (`topup_advice`); the amount is appended to the key. */
export const TOPUP_ADVICE_KEY = ['topup-advice']

/** The baskets, shared by the editor, the store field and the top-up helper. */
export const useAssetClasses = (enabled: MaybeRefOrGetter<boolean> = true) =>
  useQuery({
    queryKey: ASSET_CLASSES_KEY,
    queryFn: () => call<AssetClassesView>('asset_classes_get'),
    enabled: toRef(enabled),
  })
