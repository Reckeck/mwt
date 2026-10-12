<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'
import { toast } from 'vue-sonner'

import { CurrencyPicker } from '@/entities/currency'
import { AssetInput } from '@/entities/asset'
import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { call, parseError } from '@/shared/api/ipc'
import { tickerRef } from '@/shared/lib/asset-ref'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import type { PriceAlertDialogProps } from './price-alert-edit.types'

const props = withDefaults(defineProps<PriceAlertDialogProps>(), { alert: null, storeId: null, preset: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const asset = ref<AssetRef | null>(null)
const condition = ref<AlertCondition>('above')
const price = ref('')
const currency = ref('usd')
const active = ref(true)

/** A chart knows only the ticker: take the matching listed asset, so the alert gets a price feed. */
async function resolveTicker(symbol: string): Promise<AssetRef> {
  try {
    const hits = await call<AssetSearchHit[]>('search_assets', { query: symbol })
    const hit = hits.find((entry) => entry.symbol.toUpperCase() === symbol.toUpperCase())
    if (hit) {
      const ticker = hit.symbol.toUpperCase()
      return {
        symbol: ticker,
        name: hit.name || ticker,
        coingecko_id: hit.coingecko_id,
        chain: hit.chain,
        contract: hit.contract,
        quote: ticker,
      }
    }
  } catch {
    // Offline search: the bare ticker still names the asset.
  }
  return tickerRef(symbol)
}

watch(open, async (isOpen) => {
  if (!isOpen) return
  const preset = props.alert ? null : props.preset
  asset.value = props.alert?.asset ?? null
  condition.value = props.alert?.condition ?? preset?.condition ?? 'above'
  price.value = props.alert?.price ?? preset?.price ?? ''
  currency.value = props.alert?.currency ?? 'usd'
  active.value = props.alert?.active ?? true
  if (preset) asset.value = await resolveTicker(preset.symbol)
})

const form = useFormValidation(
  () => ({
    asset: required(asset.value, 'pick'),
    price: positiveAmount(price.value),
    currency: required(currency.value, 'pick'),
  }),
  { resetOn: open },
)

const save = useMutation({
  mutationFn: () => {
    const picked = asset.value as AssetRef
    const input: PriceAlertInput = {
      asset: {
        symbol: picked.symbol,
        name: picked.name,
        coingecko_id: picked.coingecko_id,
        chain: picked.chain,
        contract: picked.contract,
      },
      condition: condition.value,
      price: price.value.trim(),
      currency: currency.value,
      active: active.value,
      store_id: (props.alert ? props.alert.store_id : props.storeId) ?? undefined,
    }
    return props.alert
      ? call<PriceAlert>('update_price_alert', { input: { id: props.alert.id, alert: input } })
      : call<PriceAlert>('create_price_alert', { input })
  },
  onSuccess: () => {
    toast.success(props.alert ? t('alert.saved') : t('alert.created'))
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  onError: (err) => toast.error(t('alert.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.alert ? t('alert.edit') : t('alert.new')">
    <form id="price-alert-form" class="@container grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <AssetInput v-model="asset" :label="t('alert.asset')" required :error="form.error('asset')" />
      <div class="grid gap-3 @sm:grid-cols-3">
        <div class="grid gap-1.5">
          <Label for="alert-condition">{{ t('alert.condition') }}</Label>
          <Select id="alert-condition" v-model="condition">
            <option value="above">{{ t('alert.above') }}</option>
            <option value="below">{{ t('alert.below') }}</option>
          </Select>
        </div>
        <FormField for="alert-price" :label="t('alert.price')" required :error="form.error('price')">
          <AmountInput id="alert-price" v-model="price" />
        </FormField>
        <FormField for="alert-currency" :label="t('alert.currency')" required :error="form.error('currency')">
          <CurrencyPicker id="alert-currency" v-model="currency" />
        </FormField>
      </div>
      <label class="flex items-center gap-2 text-sm">
        <input v-model="active" type="checkbox" class="size-4 accent-primary" />
        {{ t('alert.active') }}
      </label>
      <p class="text-xs text-muted-foreground">{{ t('alert.hint') }}</p>
    </form>
    <template #footer>
      <Button variant="ghost" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button form="price-alert-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
