<script setup lang="ts">
import { computed, ref } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Plus, Trash2, Wallet as WalletIcon } from 'lucide-vue-next'

import { StoreCreateDialog } from '@/features/store-create'
import { StoreCard } from '@/entities/store'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { PageHeader } from '@/shared/ui/page-header'
import { SectionState } from '@/shared/ui/section-state'
import { Select, SelectItem } from '@/shared/ui/select'
import { StatCard } from '@/shared/ui/stat-card'
import { Switch } from '@/shared/ui/switch'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'
import { Money } from '@/entities/currency'
import { Dialog } from '@/shared/ui/dialog'
import { useStoreTree } from '@/shared/api/queries'
import { withCryptoShown, withoutLoyalty } from '@/shared/lib/store-tree'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const queryClient = useQueryClient()
const settings = useSettingsStore()

const tree = useStoreTree()

const allRoots = computed(() => withoutLoyalty(tree.data.value ?? []))
/** Hidden crypto wallets leave the list, not the total. */
const roots = computed(() => withCryptoShown(allRoots.value, settings.cryptoEnabled))

/* --- overview: the total, the order, archived ones on demand; both live in the URL --- */
type SortKey = 'value' | 'name' | 'kind'
const SORTS: SortKey[] = ['value', 'name', 'kind']
const sort = computed<SortKey>({
  get: () => (SORTS.includes(route.query.sort as SortKey) ? (route.query.sort as SortKey) : 'value'),
  set: (value) => void router.replace({ query: { ...route.query, sort: value === 'value' ? undefined : value } }),
})
const showArchived = computed({
  get: () => route.query.archived === '1',
  set: (value: boolean) => void router.replace({ query: { ...route.query, archived: value ? '1' : undefined } }),
})
const archivedCount = computed(() => roots.value.filter((node) => node.store.archived).length)
const total = computed(() =>
  allRoots.value.filter((node) => !node.store.archived).reduce((sum, node) => sum + Number(node.subtree_base), 0),
)
const shown = computed(() => {
  const list = roots.value.filter((node) => showArchived.value || !node.store.archived)
  const byName = (left: StoreTreeNode, right: StoreTreeNode) => left.store.name.localeCompare(right.store.name)
  if (sort.value === 'name') return [...list].sort(byName)
  if (sort.value === 'kind') return [...list].sort((left, right) => left.store.kind.localeCompare(right.store.kind) || byName(left, right))
  return [...list].sort((left, right) => Number(right.subtree_base) - Number(left.subtree_base))
})
const createDialogOpen = ref(false)
const deleting = ref<Store | null>(null)

function invalidate() {
  for (const queryKey of [['stores'], ['store'], ['summary']]) void queryClient.invalidateQueries({ queryKey })
}

const toggleArchive = useMutation({
  mutationFn: (store: Store) => call('archive_store', { id: store.id, archived: !store.archived }),
  onSuccess: invalidate,
  meta: {
    error: 'stores.archiveFailed',
    success: (_data, variables) => {
      const store = variables as Store
      return { text: store.archived ? t('stores.unarchivedToast') : t('stores.archivedToast'), description: store.name }
    },
    // `variables` is the store before the switch, so its own flag puts it back.
    undo: (_data, variables) => {
      const store = variables as Store
      return call('archive_store', { id: store.id, archived: store.archived })
    },
  },
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_store', { id }),
  onSuccess: () => {
    invalidate()
  },
  meta: { error: 'stores.deleteFailed', success: 'stores.deletedToast' },
})

function openWallet(created: Store) {
  void router.push({ name: 'wallet', params: { id: created.id } })
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <PageHeader :description="t('stores.intro')">
      <template #actions>
        <Button data-tour="stores-create" @click="createDialogOpen = true">
          <Plus class="size-4" />
          {{ t('stores.create') }}
        </Button>
      </template>
    </PageHeader>

    <SectionState
      :loading="tree.isPending.value"
      :error="tree.error.value"
      :empty="!roots.length"
      skeleton="list"
      @retry="tree.refetch()"
    >
      <div class="flex flex-wrap items-end gap-4">
        <StatCard
          class="min-w-0 flex-1 @wide:min-w-56 @wide:flex-none"
          :label="t('stores.total')"
          :hint="t('stores.totalHint')"
        >
          <Money :amount="total" :currency="settings.baseCurrency" />
        </StatCard>
        <div class="grid gap-1.5">
          <Label for="stores-sort">{{ t('stores.sort.label') }}</Label>
          <Select id="stores-sort" v-model="sort" class="w-48">
            <SelectItem v-for="key in SORTS" :key="key" :value="key">{{ t(`stores.sort.${key}`) }}</SelectItem>
          </Select>
        </div>
        <div v-if="archivedCount" class="flex h-9 items-center gap-2">
          <Switch id="stores-archived" v-model="showArchived" />
          <Label for="stores-archived">{{ t('stores.showArchived', { count: archivedCount }) }}</Label>
        </div>
      </div>

      <div class="grid items-start gap-4 @xl:grid-cols-2 @wide:grid-cols-3" data-tour="stores-list">
        <StoreCard
          v-for="node in shown"
          :key="node.store.id"
          :node="node"
          :base-currency="settings.baseCurrency"
          @toggle-archive="toggleArchive.mutate"
          @delete="(store) => (deleting = store)"
        />
      </div>

      <template #empty>
        <EmptyState :title="t('stores.empty')" :description="t('stores.emptyHint')" :icon="WalletIcon">
          <Button @click="createDialogOpen = true">
            <Plus class="size-4" />
            {{ t('stores.create') }}
          </Button>
        </EmptyState>
      </template>
    </SectionState>

    <StoreCreateDialog v-model:open="createDialogOpen" @created="openWallet" />
    <!-- A menu has no room for the two-step button: the page asks before a wallet is gone. -->
    <Dialog
      :open="deleting !== null"
      :title="t('stores.deleteConfirmTitle', { name: deleting?.name ?? '' })"
      :description="t('stores.deleteConfirmHint')"
      size="sm"
      @update:open="(value) => !value && (deleting = null)"
    >
      <template #footer>
        <Button variant="ghost" @click="deleting = null">{{ t('actions.cancel') }}</Button>
        <Button
          variant="destructive"
          :disabled="remove.isPending.value"
          @click="deleting && remove.mutate(deleting.id, { onSettled: () => (deleting = null) })"
        >
          <Trash2 class="size-4" />
          {{ t('stores.delete') }}
        </Button>
      </template>
    </Dialog>
  </div>
</template>
