<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { AssetInput } from '@/entities/asset'
import { StoreNodeSelect } from '@/entities/store'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { positiveAmount, required, useFormValidation } from '@/shared/lib/form'
import { LEDGER_KEYS } from '@/shared/lib/planning'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { TRADE_QUOTE, type TradeDraft, type TradeSide, tradeInput } from './model/trade-input'

/**
 * «Записать сделку» from Investments (Money 2 · 14): asset, exchange, side,
 * quantity and price; it writes the same spot trade as the wallet's «Спот» form.
 */
const open = defineModel<boolean>('open', { required: true })
/** Opens filled with a calculated buy. */
const props = defineProps<{ draft?: TradeDraft | null }>()

const { t } = useI18n()
const queryClient = useQueryClient()
const tree = useStoreTree()
const nodes = computed(() =>
  flattenStoreTree(tree.data.value ?? []).filter(
    (node) => !node.store.archived && node.store.kind === 'exchange' && node.store.sub_type !== 'futures',
  ),
)

const asset = ref<AssetRef | null>(null)
const storeId = ref('')
const side = ref<TradeSide>('buy')
const qty = ref('')
const price = ref('')
watch(open, (now) => {
  if (!now) return
  asset.value = props.draft?.asset ?? null
  storeId.value = nodes.value[0]?.store.id ?? ''
  side.value = 'buy'
  qty.value = props.draft?.qty ?? ''
  price.value = props.draft?.price ?? ''
})

type TradeField = 'asset' | 'store' | 'qty' | 'price'
const form = useFormValidation<TradeField>(
  () => ({
    asset: asset.value ? null : t('validation.required'),
    store: required(storeId.value, 'pick'),
    qty: positiveAmount(qty.value),
    price: positiveAmount(price.value),
  }),
  { resetOn: open },
)

const save = useMutation({
  mutationFn: () => {
    const input = asset.value && tradeInput(storeId.value, asset.value, side.value, qty.value, price.value)
    if (!input) throw { code: 'INVALID_INPUT', message: t('validation.number') }
    return call<Transaction>('add_transaction', { input })
  },
  onSuccess: () => {
    for (const queryKey of LEDGER_KEYS) void queryClient.invalidateQueries({ queryKey })
    open.value = false
  },
  meta: { error: 'form.saveFailed', success: 'trade.record.saved' },
})
</script>

<template>
  <FormDialog overlay="trade-record"
    cancel-variant="outline"
    v-model:open="open"
    form-id="trade-record"
    :title="t('trade.record.title')"
    :description="t('trade.record.hint')"
    :submit-label="t('trade.record.save')"
    :pending="save.isPending.value"
    @submit="form.submit(() => save.mutate(), $event)"
  >
    <div class="grid items-start gap-4 sm:grid-cols-2">
      <FormField for="trade-asset" :label="t('trade.record.asset')" required :error="form.error('asset')">
        <AssetInput id="trade-asset" v-model="asset" label="" :invalid="Boolean(form.error('asset'))" />
      </FormField>
      <FormField for="trade-wallet" :label="t('trade.record.wallet')" required :error="form.error('store')">
        <StoreNodeSelect id="trade-wallet" v-model="storeId" flat :nodes="nodes" :placeholder="t('trade.record.walletPick')" :invalid="Boolean(form.error('store'))" />
      </FormField>
    </div>
    <p v-if="!nodes.length" class="text-sm text-muted-foreground">{{ t('trade.record.noExchange') }}</p>
    <FormField :label="t('trade.record.side')">
      <ToggleGroup v-model="side" type="single" variant="outline" class="w-fit" :aria-label="t('trade.record.side')">
        <ToggleGroupItem value="buy" data-trade-side="buy">{{ t('trade.record.buy') }}</ToggleGroupItem>
        <ToggleGroupItem value="sell" data-trade-side="sell">{{ t('trade.record.sell') }}</ToggleGroupItem>
      </ToggleGroup>
    </FormField>
    <div class="grid items-start gap-4 sm:grid-cols-2">
      <FormField for="trade-qty" :label="t('trade.record.qty')" required :error="form.error('qty')">
        <AmountInput id="trade-qty" v-model="qty">
          <template v-if="asset" #suffix><span class="px-2 text-xs text-muted-foreground">{{ asset.symbol }}</span></template>
        </AmountInput>
      </FormField>
      <FormField for="trade-price" :label="t('trade.record.price')" required :error="form.error('price')">
        <AmountInput id="trade-price" v-model="price">
          <template #suffix><span class="px-2 text-xs text-muted-foreground">{{ TRADE_QUOTE.symbol }}</span></template>
        </AmountInput>
      </FormField>
    </div>
  </FormDialog>
</template>
