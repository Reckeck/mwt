<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { AlertTriangle, Minus, Plus, Trash2 } from 'lucide-vue-next'

import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { AmountInput } from '@/shared/ui/amount-input'
import { Input } from '@/shared/ui/input'
import { AssetSelect } from '@/entities/asset'
import { assetKey } from '@/shared/lib/asset-ref'
import { nonNegativeAmount, required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { formatQuantity } from '@/shared/lib/format'

import type { EnvelopeManagerProps } from './envelope-manage.types'
import { ProgressBar } from '@/shared/ui/progress-bar'

const props = defineProps<EnvelopeManagerProps>()
const { t } = useI18n()
const queryClient = useQueryClient()

const envelopes = useQuery({
  // no placeholder: another wallet's envelopes
  queryKey: computed(() => ['envelopes', props.storeId]),
  queryFn: () => call<EnvelopeView[]>('list_envelopes', { storeId: props.storeId }),
})

const refresh = () => {
  void queryClient.invalidateQueries({ queryKey: ['envelopes'] })
  void queryClient.invalidateQueries({ queryKey: ['goals'] })
}

/* --- new envelope --- */
const name = ref('')
const pickedAsset = ref<AssetRef | null>(null)
const target = ref('')

const form = useFormValidation(() => ({
  name: required(name.value),
  asset: required(pickedAsset.value, 'pick'),
  target: nonNegativeAmount(target.value),
}))
watch(
  () => props.assets,
  (assets) => {
    const current = pickedAsset.value ? assetKey(pickedAsset.value) : ''
    if (!assets.some((asset) => assetKey(asset) === current)) pickedAsset.value = assets[0] ?? null
  },
  { immediate: true },
)

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
    form.reset()
    target.value = ''
    refresh()
  },
  meta: { success: 'envelope.created', error: 'envelope.failed' },
})

/* --- earmark / release, one amount box per row --- */
const amounts = ref<Record<string, string>>({})

const fund = useMutation({
  mutationFn: ({ id, amount }: { id: string; amount: string }) => call('fund_envelope', { input: { id, amount } }),
  onSuccess: (_result, { id }) => {
    amounts.value[id] = ''
    refresh()
  },
  meta: { success: 'envelope.funded', error: 'envelope.failed' },
})

const remove = useMutation({
  mutationFn: (id: string) => call('delete_envelope', { id }),
  onSuccess: refresh,
  meta: { success: 'envelope.deleted', error: 'envelope.failed' },
})

function apply(id: string, sign: 1 | -1) {
  const raw = (amounts.value[id] ?? '').trim().replace(',', '.')
  if (!raw || Number(raw) <= 0) return
  fund.mutate({ id, amount: sign < 0 ? `-${raw}` : raw })
}

/** Envelopes hold an asset, not money in the base currency: a quantity and its ticker. */
const qty = (value: string, symbol: string) => `${formatQuantity(value)} ${symbol.toUpperCase()}`
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
            :confirm-label="t('actions.confirmDeleteNamed', { name: view.envelope.name })"
            @confirm="remove.mutate(view.envelope.id)"
          >
            <Trash2 class="size-4" />
          </ConfirmButton>
        </div>
        <ProgressBar v-if="view.pct !== null" :value="view.pct" size="sm" />
        <p v-if="view.short" class="flex items-center gap-1.5 text-xs text-warning">
          <AlertTriangle class="size-3.5" />
          {{ t('envelope.short', { free: qty(view.free, view.envelope.asset.symbol) }) }}
        </p>
        <div class="flex flex-wrap items-center gap-2">
          <AmountInput
            v-model="amounts[view.envelope.id]"
            class="h-8 w-32"
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
      @submit.prevent="form.submit(create.mutate, $event)"
    >
      <p class="text-sm font-medium">{{ t('envelope.new') }}</p>
      <div class="grid gap-3 @mid:grid-cols-[1fr_8rem_8rem_auto] @mid:items-start">
        <FormField for="envelope-name" :label="t('envelope.name')" required :error="form.error('name')">
          <Input id="envelope-name" v-model="name" :placeholder="t('envelope.namePlaceholder')" />
        </FormField>
        <FormField for="envelope-asset" :label="t('form.asset')" required :error="form.error('asset')">
          <AssetSelect
            id="envelope-asset"
            v-model="pickedAsset"
            :assets="props.assets"
            :invalid="Boolean(form.error('asset'))"
          />
        </FormField>
        <FormField for="envelope-target" :label="t('envelope.target')" :error="form.error('target')">
          <AmountInput id="envelope-target" v-model="target" />
        </FormField>
        <Button type="submit" class="@mid:mt-[1.375rem]" :disabled="create.isPending.value">
          <Plus class="size-4" />
          {{ t('envelope.add') }}
        </Button>
      </div>
    </form>
    <p v-else class="text-sm text-muted-foreground">{{ t('envelope.noAssets') }}</p>
  </div>
</template>
