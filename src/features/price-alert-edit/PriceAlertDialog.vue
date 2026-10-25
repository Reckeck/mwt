<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { CurrencyPicker, Money } from '@/entities/currency'
import { AssetInput, resolveTicker } from '@/entities/asset'
import { FormField } from '@/shared/ui/form-field'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { AmountInput } from '@/shared/ui/amount-input'
import { Select, SelectItem } from '@/shared/ui/select'
import { SwitchRow } from '@/shared/ui/switch-row'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'

import type { PriceAlertDialogProps } from './price-alert-edit.types'
import { useCurrentPrice } from './model/use-current-price'

/**
 * «Новый алерт цены» (Auto-2 · 11): asset and condition side by side, the
 * level with its currency inside the field and today's price under it, and
 * whether it fires once or on every crossing.
 */
const props = withDefaults(defineProps<PriceAlertDialogProps>(), { alert: null, storeId: null, preset: null })
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const queryClient = useQueryClient()

const asset = ref<AssetRef | null>(null)
const condition = ref<AlertCondition>('above')
const price = ref('')
const currency = ref('usd')
const once = ref(true)

watch(open, async (isOpen) => {
  if (!isOpen) return
  const preset = props.alert ? null : props.preset
  asset.value = props.alert?.asset ?? null
  condition.value = props.alert?.condition ?? preset?.condition ?? 'above'
  price.value = props.alert?.price ?? preset?.price ?? ''
  currency.value = props.alert?.currency ?? 'usd'
  once.value = !props.alert?.repeats
  if (preset) asset.value = await resolveTicker(preset.symbol)
}, { immediate: true })

const current = useCurrentPrice(asset, currency)

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
      // Switched off from «Мои»; the dialog keeps it as it was.
      active: props.alert?.active ?? true,
      store_id: (props.alert ? props.alert.store_id : props.storeId) ?? undefined,
      repeats: !once.value,
    }
    return props.alert
      ? call<PriceAlert>('update_price_alert', { input: { id: props.alert.id, alert: input } })
      : call<PriceAlert>('create_price_alert', { input })
  },
  onSuccess: () => {
    for (const queryKey of PLANNING_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'alert.saveFailed', success: () => (props.alert ? t('alert.saved') : t('alert.created')) },
})
</script>

<template>
  <Dialog v-model:open="open" :title="props.alert ? t('alert.edit') : t('alert.newPrice')" size="form">
    <form id="price-alert-form" class="grid gap-4" @submit.prevent="form.submit(save.mutate, $event)">
      <div class="grid items-start gap-4 sm:grid-cols-2">
        <AssetInput v-model="asset" :label="t('alert.asset')" required :error="form.error('asset')" />
        <FormField for="alert-condition" :label="t('alert.condition')" required>
          <Select id="alert-condition" v-model="condition">
            <SelectItem value="above">{{ t('alert.priceAbove') }}</SelectItem>
            <SelectItem value="below">{{ t('alert.priceBelow') }}</SelectItem>
          </Select>
        </FormField>
      </div>
      <FormField
        for="alert-price"
        :label="t('alert.value')"
        required
        :error="form.error('price') ?? form.error('currency')"
      >
        <template v-if="current !== null" #hint>
          <i18n-t keypath="alert.now">
            <template #price><Money :amount="current" :currency="currency" /></template>
          </i18n-t>
        </template>
        <AmountInput id="alert-price" v-model="price" size="lg">
          <template #suffix>
            <CurrencyPicker id="alert-currency" v-model="currency" label="" chip />
          </template>
        </AmountInput>
      </FormField>
      <SwitchRow id="alert-once" v-model="once" :label="t('alert.once')" />
    </form>
    <template #footer>
      <Button variant="ghost" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button class="max-md:h-11 max-md:flex-1" form="price-alert-form" type="submit" :disabled="save.isPending.value">{{ t('actions.save') }}</Button>
    </template>
  </Dialog>
</template>
