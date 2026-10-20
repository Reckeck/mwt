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
