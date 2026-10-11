<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { AlertTriangle, Minus, Plus, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call, parseError } from '@/shared/api/ipc'
import { formatMoney } from '@/shared/lib/format'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { EnvelopeManagerProps } from './envelope-manage.types'

const props = defineProps<EnvelopeManagerProps>()
const { t } = useI18n()
const queryClient = useQueryClient()

const envelopes = useQuery({
  queryKey: computed(() => ['envelopes', props.storeId]),
  queryFn: () => call<EnvelopeView[]>('list_envelopes', { storeId: props.storeId }),
})

const refresh = () => {
  void queryClient.invalidateQueries({ queryKey: ['envelopes'] })
  void queryClient.invalidateQueries({ queryKey: ['goals'] })
}
const fail = (err: unknown) => toast.error(t('envelope.failed'), { description: codeErrorMessage(parseError(err), t) })

/* --- new envelope --- */
const name = ref('')
const assetKey = ref('')
const target = ref('')
watch(
  () => props.assets,
  (assets) => {
    if (!assets.some((asset) => asset.symbol === assetKey.value)) assetKey.value = assets[0]?.symbol ?? ''
  },
  { immediate: true },
)
const pickedAsset = computed(() => props.assets.find((asset) => asset.symbol === assetKey.value) ?? null)

const create = useMutation({
  mutationFn: () =>
    call<Envelope>('create_envelope', {
      input: {
        store_id: props.storeId,
        name: name.value.trim(),
        asset: pickedAsset.value as AssetRef,
        target: target.value.trim() || null,
        goal_id: null,
      } satisfies EnvelopeDraft,
    }),
  onSuccess: () => {
    name.value = ''
    target.value = ''
    toast.success(t('envelope.created'))
    refresh()
  },
  onError: fail,
})

/* --- earmark / release, one amount box per row --- */
const amounts = ref<Record<string, string>>({})

const fund = useMutation({
  mutationFn: ({ id, amount }: { id: string; amount: string }) => call('fund_envelope', { input: { id, amount } }),
  onSuccess: (_result, { id }) => {
    amounts.value[id] = ''
    refresh()
  },
  onError: fail,
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_envelope', { id }),
  onSuccess: refresh,
  onError: fail,
})

function apply(id: string, sign: 1 | -1) {
  const raw = (amounts.value[id] ?? '').trim().replace(',', '.')
  if (!raw || Number(raw) <= 0) return
  fund.mutate({ id, amount: sign < 0 ? `-${raw}` : raw })
}

const qty = (value: string, symbol: string) => `${formatMoney(value, { decimals: 2 })} ${symbol.toUpperCase()}`
</script>

<template>
  <div class="grid gap-4" data-tour="wallet-envelopes">
    <p class="text-sm text-muted-foreground">{{ t('envelope.hint') }}</p>

    <ul v-if="envelopes.data.value?.length" class="grid gap-3">
      <li
        v-for="view in envelopes.data.value"
        :key="view.envelope.id"
        class="grid gap-2 rounded-lg border border-border p-3"
      >
        <div class="flex flex-wrap items-center gap-2">
          <span class="min-w-0 flex-1 truncate text-sm font-medium">{{ view.envelope.name }}</span>
          <span class="text-sm tabular-nums">
            {{ qty(view.envelope.allocated, view.envelope.asset.symbol) }}
            <template v-if="view.envelope.target">
              / {{ qty(view.envelope.target, view.envelope.asset.symbol) }}
            </template>
          </span>
          <ConfirmButton
            :label="t('envelope.delete')"
            :confirm-label="t('actions.confirmDelete')"
            @confirm="remove.mutate(view.envelope.id)"
          >
            <Trash2 class="size-4" />
          </ConfirmButton>
        </div>
        <div v-if="view.pct !== null" class="h-1.5 overflow-hidden rounded-full bg-muted">
          <div class="h-full rounded-full bg-primary" :style="{ width: `${Math.min(100, view.pct)}%` }" />
        </div>
        <p v-if="view.short" class="flex items-center gap-1.5 text-xs text-amber-600 dark:text-amber-400">
          <AlertTriangle class="size-3.5" />
          {{ t('envelope.short', { free: qty(view.free, view.envelope.asset.symbol) }) }}
        </p>
        <div class="flex flex-wrap items-center gap-2">
          <Input
            v-model="amounts[view.envelope.id]"
            class="h-8 w-32"
            inputmode="decimal"
            :placeholder="t('envelope.amount')"
            :aria-label="t('envelope.amount')"
          />
          <Button size="sm" variant="outline" :disabled="fund.isPending.value" @click="apply(view.envelope.id, 1)">
            <Plus class="size-4" />
            {{ t('envelope.fund') }}
          </Button>
          <Button size="sm" variant="ghost" :disabled="fund.isPending.value" @click="apply(view.envelope.id, -1)">
            <Minus class="size-4" />
            {{ t('envelope.release') }}
          </Button>
          <span class="ml-auto text-xs text-muted-foreground">
            {{ t('envelope.free', { free: qty(view.free, view.envelope.asset.symbol) }) }}
          </span>
        </div>
      </li>
    </ul>

    <form
      v-if="props.assets.length"
      class="@container grid gap-3 rounded-lg border border-dashed border-border p-3"
      @submit.prevent="create.mutate()"
    >
      <p class="text-sm font-medium">{{ t('envelope.new') }}</p>
      <div class="grid gap-3 @md:grid-cols-[1fr_8rem_8rem_auto] @md:items-end">
        <div class="grid gap-1.5">
          <Label for="envelope-name">{{ t('envelope.name') }}</Label>
          <Input id="envelope-name" v-model="name" :placeholder="t('envelope.namePlaceholder')" />
        </div>
        <div class="grid gap-1.5">
          <Label for="envelope-asset">{{ t('form.asset') }}</Label>
          <Select id="envelope-asset" v-model="assetKey">
            <option v-for="asset in props.assets" :key="asset.symbol" :value="asset.symbol">{{ asset.symbol }}</option>
          </Select>
        </div>
        <div class="grid gap-1.5">
          <Label for="envelope-target">{{ t('envelope.target') }}</Label>
          <Input id="envelope-target" v-model="target" inputmode="decimal" />
        </div>
        <Button type="submit" :disabled="!name.trim() || !pickedAsset || create.isPending.value">
          <Plus class="size-4" />
          {{ t('envelope.add') }}
        </Button>
      </div>
    </form>
    <p v-else class="text-sm text-muted-foreground">{{ t('envelope.noAssets') }}</p>
  </div>
</template>
