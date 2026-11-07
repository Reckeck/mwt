<script setup lang="ts">
import { computed, ref } from 'vue'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { StoreNodeSelect } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { SectionState } from '@/shared/ui/section-state'
import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { notify } from '@/shared/lib/notify'

import ExchangeKeyEntry from './ExchangeKeyEntry.vue'
import ExchangeKeyNotice from './ExchangeKeyNotice.vue'
import { useExchangeKeyState } from './use-exchange-key-state'
import type { ExchangeKeyFormProps } from './exchange-key.types'
import { useExchangeKeyStatus } from './useExchangeKeyStatus'
import { useStoreTree } from '@/shared/api/queries'

const props = defineProps<ExchangeKeyFormProps>()

const { t } = useI18n()
const queryClient = useQueryClient()

const { status, hasKey, hint } = useExchangeKeyStatus(
  computed(() => props.storeId),
  computed(() => true),
)

const { state: keyState, notVerified, levelRequired } = useExchangeKeyState(
  computed(() => props.storeId),
  computed(() => true),
)
const busy = ref(false)

/** After a key is stored or refused: the tree (new sub-accounts), the status and the state. */
function refresh() {
  for (const queryKey of [STORE_TREE_QUERY_KEY, ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
  void status.refetch()
  void keyState.refetch()
}

const tree = useStoreTree()

/** Exchange sub-accounts of this wallet: where imported fills may land. */
const targets = computed(() => {
  const nodes = flattenStoreTree(tree.data.value ?? [])
  const self = nodes.find((node) => node.store.id === props.storeId)
  if (!self) return []
  const prefix = self.path.join('\u0000')
  return nodes.filter(
    (node) =>
      node.store.id !== props.storeId &&
      node.store.kind === 'exchange' &&
      !node.store.archived &&
      node.path.join('\u0000').startsWith(`${prefix}\u0000`),
  )
})

const setTargets = useMutation({
  mutationFn: (patch: { spot_store_id?: string | null; futures_store_id?: string | null }) =>
    call<ExchangeKeyStatus>('set_exchange_targets', {
      input: {
        store_id: props.storeId,
        spot_store_id: status.data.value?.spot_store_id ?? null,
        futures_store_id: status.data.value?.futures_store_id ?? null,
        ...patch,
      },
    }),
  onSuccess: () => {
    void status.refetch()
  },
  meta: { error: 'settings.saveFailed', success: 'settings.saved' },
})

/* --- Bybit's own split: Funding, Unified Trading, Earn --- */
const ACCOUNTS = ['funding', 'unified', 'earn'] as const
const accounts = computed(() =>
  ACCOUNTS.map((market) => ({
    market,
    node: targets.value.find(
      (node) => typeof node.store.binding === 'object' && 'market' in node.store.binding && node.store.binding.market === market,
    ),
  })),
)
const missingAccounts = computed(() => accounts.value.filter((entry) => !entry.node).length)
const createAccounts = useMutation({
  mutationFn: () =>
    call<string[]>('create_bybit_accounts', {
      input: {
        store_id: props.storeId,
        funding_name: t('subType.funding'),
        unified_name: t('subType.unified'),
        earn_name: t('subType.earn'),
        detect: false,
      } satisfies CreateBybitAccountsInput,
    }),
  onSuccess: () => {
    for (const queryKey of [STORE_TREE_QUERY_KEY, ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
  },
  meta: { error: 'settings.saveFailed', success: 'wallet.bybit.accountsCreated' },
})

async function remove() {
  busy.value = true
  try {
    await call('clear_exchange_key', { input: { store_id: props.storeId } })
    notify.success('wallet.bybit.keyRemoved')
    await Promise.all([status.refetch(), keyState.refetch()])
  } catch (err) {
    notify.error(err, 'wallet.bybit.keyRemoveFailed')
  } finally {
    busy.value = false
  }
}
</script>

<template>
  <div class="flex flex-col gap-4">
    <p class="text-sm text-muted-foreground">{{ t('wallet.bybit.hint') }}</p>

    <!-- Imports land where Bybit keeps the money: one sub-account per Bybit account. -->
    <div class="grid gap-2 rounded-lg border border-border p-3">
      <p class="text-sm font-medium">{{ t('wallet.bybit.accounts') }}</p>
      <ul class="grid gap-1 text-sm">
        <li v-for="entry in accounts" :key="entry.market" class="flex items-center justify-between gap-2">
          <span>{{ t(`subType.${entry.market}`) }}</span>
          <Badge :variant="entry.node ? 'secondary' : 'outline'">
            {{ entry.node ? entry.node.store.name : t('wallet.bybit.accountMissing') }}
          </Badge>
        </li>
      </ul>
      <p class="text-xs text-muted-foreground">{{ t('wallet.bybit.accountsHint') }}</p>
      <Button
        v-if="missingAccounts"
        size="sm"
        variant="outline"
        class="w-fit"
        :disabled="createAccounts.isPending.value"
        @click="createAccounts.mutate()"
      >
        {{ t('wallet.bybit.createAccounts', { count: missingAccounts }) }}
      </Button>
    </div>

    <SectionState
      :loading="keyState.isPending.value"
      :error="keyState.error.value"
      :rows="2"
      compact
      @retry="keyState.refetch()"
    >
      <ExchangeKeyNotice
        :state="keyState.data.value ?? 'missing'"
        :connected="hasKey"
        :elsewhere="!hasKey && missingAccounts < accounts.length"
      />
      <div v-if="hasKey && !levelRequired" class="grid gap-3">
        <div class="flex flex-wrap items-center gap-2">
          <Badge variant="secondary">{{ t('securityKeys.masked', { hint }) }}</Badge>
          <Badge v-if="notVerified" variant="warning">{{ t('securityKeys.notVerified') }}</Badge>
          <Button variant="ghost" :disabled="busy" @click="remove">
            {{ t('wallet.bybit.remove') }}
          </Button>
        </div>
        <p v-if="notVerified" class="text-sm text-muted-foreground">{{ t('securityKeys.notVerifiedText') }}</p>
        <!-- Where confirmed fills are booked: the wallet itself or one of its exchange sub-accounts. -->
        <div v-if="targets.length" class="grid gap-3 sm:grid-cols-2">
          <div class="grid gap-1.5">
            <Label for="bybit-spot-target">{{ t('wallet.bybit.spotTarget') }}</Label>
            <StoreNodeSelect
              id="bybit-spot-target"
              :model-value="status.data.value?.spot_store_id ?? ''"
              :nodes="targets"
              :empty-label="t('wallet.bybit.thisWallet')"
              @update:model-value="(id) => setTargets.mutate({ spot_store_id: id || null })"
            />
          </div>
          <div class="grid gap-1.5">
            <Label for="bybit-futures-target">{{ t('wallet.bybit.futuresTarget') }}</Label>
            <StoreNodeSelect
              id="bybit-futures-target"
              :model-value="status.data.value?.futures_store_id ?? ''"
              :nodes="targets"
              :empty-label="t('wallet.bybit.thisWallet')"
              @update:model-value="(id) => setTargets.mutate({ futures_store_id: id || null })"
            />
          </div>
        </div>
      </div>
      <ExchangeKeyEntry v-else-if="!levelRequired" :store-id="props.storeId" @saved="refresh" @refused="refresh" />
    </SectionState>
  </div>
</template>
