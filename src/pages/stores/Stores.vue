<script setup lang="ts">
import { ref } from 'vue'
import { useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { Plus, Trash2, Wallet as WalletIcon } from 'lucide-vue-next'

import { StoreCreateDialog } from '@/features/store-create'
import { CapitalGlance } from '@/widgets/quick-glance'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { EmptyState } from '@/shared/ui/empty-state'
import { SectionState } from '@/shared/ui/section-state'

import { useWalletTable } from './model/use-wallet-table'
import WalletAssets from './ui/WalletAssets.vue'
import WalletList from './ui/WalletList.vue'
import WalletTable from './ui/WalletTable.vue'
import WalletsHeader from './ui/WalletsHeader.vue'

/**
 * Wallets (Wallets-1 · 1, Currency-1 · 2): one total over a table — each wallet
 * in its own currency, the sum in the base (or the budget's currency), what the
 * wallets hold per currency under it. A phone gets a list.
 */
const { t } = useI18n()
const router = useRouter()
const queryClient = useQueryClient()
const phone = useIsPhone()
const table = useWalletTable()

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
  onSuccess: invalidate,
  meta: { error: 'stores.deleteFailed', success: 'stores.deletedToast' },
})

function openWallet(created: Store) {
  void router.push({ name: 'wallet', params: { id: created.id } })
}
</script>

<template>
  <div class="grid grid-cols-1 gap-4">
    <SectionState
      :loading="table.tree.isPending.value"
      :error="table.tree.error.value"
      :empty="!table.roots.value.length"
      skeleton="list"
      @retry="table.tree.refetch()"
    >
      <WalletsHeader
        v-model:mode="table.mode.value"
        :phone="phone"
        :second="table.second.value"
        :base="table.base.value"
        :rates="table.rates.value"
        @create="createDialogOpen = true"
      >
        <CapitalGlance trigger-class="text-3xl leading-tight font-semibold tabular-nums">
          <Money
            v-if="table.shown(table.total.value) !== null"
            :amount="table.shown(table.total.value) ?? 0"
            :currency="table.currency.value"
          />
          <span v-else>—</span>
        </CapitalGlance>
      </WalletsHeader>

      <WalletAssets v-if="table.mode.value === 'assets' && !phone" table :assets="table.assets.value" :base="table.base.value" />
      <WalletList v-else-if="phone" :rows="table.rows.value" :currency="table.currency.value" :shown="table.shown" />
      <WalletTable
        v-else
        v-model:sort="table.sort.value"
        :rows="table.rows.value"
        :currency="table.currency.value"
        :shown="table.shown"
        @toggle-archive="toggleArchive.mutate"
        @delete="(store) => (deleting = store)"
      />
      <WalletAssets
        v-if="table.mode.value !== 'assets' && !phone && table.assets.value.length > 1"
        :assets="table.assets.value"
        :base="table.base.value"
      />

      <p v-if="phone && table.rates.value" class="text-center text-xs text-muted-foreground">{{ table.rates.value }}</p>
      <p v-else-if="!phone" class="flex flex-wrap items-center gap-x-2 text-xs text-muted-foreground">
        {{ t('stores.footer') }}
        <Button
          v-if="table.archivedCount.value"
          variant="link"
          size="sm"
          class="h-auto p-0 text-xs"
          @click="table.showArchived.value = !table.showArchived.value"
        >
          {{ table.showArchived.value ? t('stores.hideArchived') : t('stores.showArchived', { count: table.archivedCount.value }) }}
        </Button>
      </p>

      <template #empty>
        <EmptyState :title="t('stores.empty')" :description="t('stores.emptyHint')" :icon="WalletIcon">
          <Button @click="createDialogOpen = true">
            <Plus class="size-4" />
            {{ t('stores.createShort') }}
          </Button>
        </EmptyState>
      </template>
    </SectionState>

    <StoreCreateDialog v-model:open="createDialogOpen" @created="openWallet" />
    <!-- A menu has no room for the two-step button: the page asks before a wallet is gone. -->
    <Dialog overlay="store-delete"
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
