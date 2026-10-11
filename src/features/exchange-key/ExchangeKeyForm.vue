<script setup lang="ts">
import { computed, ref } from 'vue'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'
import { toast } from 'vue-sonner'

import { StoreNodeSelect } from '@/entities/store'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { call, parseError } from '@/shared/api/ipc'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { connectBybit } from './connect-bybit'
import type { ExchangeKeyFormProps } from './exchange-key.types'
import { useExchangeKeyStatus } from './useExchangeKeyStatus'

const props = defineProps<ExchangeKeyFormProps>()

const { t } = useI18n()
const queryClient = useQueryClient()

const { status, hasKey, hint } = useExchangeKeyStatus(
  computed(() => props.storeId),
  computed(() => true),
)

const apiKey = ref('')
const apiSecret = ref('')
const busy = ref(false)

async function save() {
  busy.value = true
  try {
    const count = await connectBybit(props.storeId, apiKey.value, apiSecret.value, {
      funding: t('subType.funding'),
      unified: t('subType.unified'),
      earn: t('subType.earn'),
    })
    apiKey.value = ''
    apiSecret.value = ''
    toast.success(t('wallet.bybit.keySaved'), { description: t('wallet.bybit.accountsReady', { count }) })
    for (const queryKey of [STORE_TREE_QUERY_KEY, ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
    await status.refetch()
  } catch (err) {
    toast.error(t('wallet.bybit.keySaveFailed'), { description: parseError(err).message })
  } finally {
    busy.value = false
  }
}

const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })

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
    toast.success(t('settings.saved'))
    void status.refetch()
  },
  onError: (err) => toast.error(t('settings.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
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
    toast.success(t('wallet.bybit.accountsCreated'))
    for (const queryKey of [STORE_TREE_QUERY_KEY, ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
  },
  onError: (err) => toast.error(t('settings.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})

async function remove() {
  busy.value = true
  try {
    await call('clear_exchange_key', { input: { store_id: props.storeId } })
    toast.success(t('wallet.bybit.keyRemoved'))
    await status.refetch()
  } catch (err) {
    toast.error(t('wallet.bybit.keyRemoveFailed'), { description: parseError(err).message })
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

    <template v-if="hasKey">
      <div class="flex items-center gap-2">
        <Badge variant="secondary">{{ hint }}</Badge>
        <Button variant="ghost" :disabled="busy" @click="remove">
          {{ t('wallet.bybit.remove') }}
        </Button>
      </div>
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
    </template>

    <!-- Key, secret and the button on one row: it is a two-field form. -->
    <form v-else class="grid items-end gap-3 sm:grid-cols-[1fr_1fr_auto]" @submit.prevent="save">
      <div class="grid gap-1.5">
        <Label for="bk">{{ t('wallet.bybit.apiKey') }}</Label>
        <Input id="bk" v-model="apiKey" autocomplete="off" spellcheck="false" />
      </div>
      <div class="grid gap-1.5">
        <Label for="bs">{{ t('wallet.bybit.apiSecret') }}</Label>
        <Input id="bs" v-model="apiSecret" type="password" autocomplete="new-password" spellcheck="false" />
      </div>
      <Button type="submit" :disabled="busy || !apiKey || !apiSecret">
        {{ t('wallet.bybit.save') }}
      </Button>
    </form>
  </div>
</template>
