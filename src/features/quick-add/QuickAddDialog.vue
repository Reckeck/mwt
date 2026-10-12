<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { TransactionForm } from '@/features/transaction-create'
import { StoreNodeSelect } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Label } from '@/shared/ui/label'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'

/** «+ Операция» from anywhere: the wallet is the first field, not the page's. */
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const LAST_STORE_KEY = 'quick-add.store'

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const nodes = computed(() => flattenStoreTree(tree.data.value ?? []).filter((node) => !node.store.archived))

function remembered(): string {
  try {
    return localStorage.getItem(LAST_STORE_KEY) ?? ''
  } catch {
    return ''
  }
}

const storeId = ref('')
// Each opening starts on the wallet used last time, if it still exists.
watch(
  [open, nodes],
  ([isOpen, list]) => {
    if (!isOpen || (storeId.value && list.some((node) => node.store.id === storeId.value))) return
    const last = remembered()
    storeId.value = list.find((node) => node.store.id === last)?.store.id ?? list[0]?.store.id ?? ''
  },
  { immediate: true },
)

const form = ref<InstanceType<typeof TransactionForm> | null>(null)

function onSaved() {
  try {
    localStorage.setItem(LAST_STORE_KEY, storeId.value)
  } catch {
    // Private mode: the choice is simply not remembered.
  }
  open.value = false
}
</script>

<template>
  <Dialog v-model:open="open" :title="t('quickAdd.title')" size="lg">
    <div class="@container grid gap-4">
      <p v-if="tree.isSuccess.value && !nodes.length" class="text-sm text-muted-foreground">{{ t('quickAdd.noStores') }}</p>
      <template v-else>
        <div class="grid gap-1.5">
          <Label for="quick-add-store">{{ t('quickAdd.store') }}</Label>
          <StoreNodeSelect id="quick-add-store" v-model="storeId" :nodes="nodes" class="w-full" />
        </div>
        <!-- Keyed by the wallet: the form fills its fields from it once. -->
        <TransactionForm v-if="open && storeId" :key="storeId" ref="form" :store-id="storeId" @saved="onSaved" />
      </template>
    </div>

    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="transaction-form" type="submit" :disabled="form?.pending">
        {{ t('form.submit') }}
      </Button>
    </template>
  </Dialog>
</template>
