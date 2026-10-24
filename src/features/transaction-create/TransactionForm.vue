<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { TxPreviewCard } from '@/entities/transaction'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'

import { provideTxForm } from './model/use-tx-form'
import { formToInput } from './transaction-form.model'
import type { TransactionFormProps } from './transaction-form.types'
import { TxAssetSellSection } from './ui/tx-asset-sell-section'
import { TxCashSection } from './ui/tx-cash-section'
import { TxCommonFields } from './ui/tx-common-fields'
import { TxConvertSection } from './ui/tx-convert-section'
import { TxFillsList } from './ui/tx-fills-list'
import { TxFuturesSection } from './ui/tx-futures-section'
import { TxKindPicker } from './ui/tx-kind-picker'
import { TxKindTabs } from './ui/tx-kind-tabs'
import { TxMissingSummary } from './ui/tx-missing-summary'
import { TxQuickCash } from './ui/tx-quick-cash'
import { TxQuickConvert } from './ui/tx-quick-convert'
import { TxQuickDefi } from './ui/tx-quick-defi'
import { TxQuickFutures } from './ui/tx-quick-futures'
import { TxQuickMore } from './ui/tx-quick-more'
import { TxQuickSpot } from './ui/tx-quick-spot'
import { TxQuickTransfer } from './ui/tx-quick-transfer'
import { TxSpotSection } from './ui/tx-spot-section'
import { TxTransferSection } from './ui/tx-transfer-section'

/**
 * One operation of any kind: the kind's own fields, then what every kind
 * shares, then the summary and the balance preview. The logic lives in
 * `model/use-tx-*` (put together by `use-tx-form`), the sections in `ui/tx-*`.
 * `compact` is the short form of Ctrl N (OpForm-1): the dialog's wallet field
 * comes in through `#store`.
 */
const props = withDefaults(defineProps<TransactionFormProps>(), {
  transaction: null,
  plan: null,
  disabled: false,
  counterpart: null,
  template: false,
  flowId: null,
  prefill: null,
  compact: false,
})
defineSlots<{ store?: () => unknown; hint?: () => unknown }>()

/** `saved` after every successful write; `submit` instead in template mode. */
const emit = defineEmits<{ saved: [tx: Transaction]; submit: [] }>()

/** Bumped after each save: the next operation in a series starts with fresh pickers and no errors. */
const round = ref(0)
const { form, check, save, valid, showFills, debouncedDraft, preview, isProperty } = provideTxForm(props, (tx) => {
  check.reset()
  round.value += 1
  emit('saved', tx)
})

/** Payment and top-up take the short layout; a property's own top-up keeps its unit fields. */
const quickCash = computed(() => props.compact && (form.kind === 'payment' || form.kind === 'topup') && !isProperty.value)
/** Exchange and transfer take their wallets as sides, so the dialog's wallet field is not shown. */
const quickSides = computed(() => (props.compact && ['convert', 'transfer'].includes(form.kind) ? form.kind : null))
/** Spot, futures and DeFi of «Сделка · <биржа>» (OpForm-2 · 9–11): the exchange's own wallet, no comment line. */
const quickDeal = computed(() =>
  props.compact && ['trade_spot', 'trade_futures', 'defi'].includes(form.kind) ? form.kind : null,
)

const { t } = useI18n()

function submit() {
  if (props.template) emit('submit')
  else save.mutate()
}

/** The dialog owns the submit button, so it reads these; a template's parent reads `input`. */
defineExpose({
  valid,
  /** For a parent that owns the save button of a template: show the messages under the fields. */
  showErrors: () => {
    check.attempted.value = true
  },
  pending: computed(() => save.isPending.value),
  input: computed(() => formToInput(form)),
  /** What the user typed, for the next form when the dialog swaps the wallet. */
  draft: computed(() => ({
    kind: form.kind,
    categories: [...form.categories],
    amount: form.amount,
    comment: form.comment,
    ...(form.to && 'store' in form.to ? { to: form.to.store } : {}),
  })),
})
</script>

<template>
  <form
    :id="props.template ? 'transaction-template-form' : 'transaction-form'"
    class="flex flex-col gap-4"
    @submit.prevent="check.submit(submit, $event)"
  >
    <TxKindTabs v-if="props.compact" />
    <TxKindPicker v-else />
    <slot name="hint" />

    <TxQuickCash v-if="quickCash" :key="round">
      <template #store><slot name="store" /></template>
    </TxQuickCash>
    <TxQuickConvert v-else-if="quickSides === 'convert'" :key="`c${round}`" />
    <TxQuickTransfer v-else-if="quickSides === 'transfer'" :key="`t${round}`" />
    <TxQuickSpot v-else-if="quickDeal === 'trade_spot'" :key="`s${round}`" />
    <TxQuickFutures v-else-if="quickDeal === 'trade_futures'" :key="`f${round}`" />
    <TxQuickDefi v-else-if="quickDeal === 'defi'" :key="`d${round}`" />
    <template v-else>
      <slot v-if="props.compact" name="store" />
      <TxCashSection v-if="form.kind === 'topup' || form.kind === 'payment' || form.kind === 'defi'" />
      <TxTransferSection v-else-if="form.kind === 'transfer'" />
      <TxConvertSection v-else-if="form.kind === 'convert'" />
      <TxSpotSection v-else-if="form.kind === 'trade_spot'" />
      <TxFuturesSection v-else-if="form.kind === 'trade_futures'" />
      <TxAssetSellSection v-else-if="form.kind === 'asset_sell'" />
    </template>

    <TxFillsList v-if="showFills" />
    <template v-if="props.compact">
      <FormField v-if="quickSides !== 'convert' && !quickDeal" for="tx-comment" :label="t('form.comment')">
        <Input id="tx-comment" v-model="form.comment" autocomplete="off" :placeholder="t('form.optional')" />
      </FormField>
      <TxQuickMore />
    </template>
    <template v-else>
      <TxCommonFields />
      <TxMissingSummary />
      <TxPreviewCard v-if="debouncedDraft" :preview="preview.data.value" :loading="preview.isFetching.value" />
    </template>
  </form>
</template>
