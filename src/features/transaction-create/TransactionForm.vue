<script setup lang="ts">
import { computed } from 'vue'

import { TxPreviewCard } from '@/entities/transaction'

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
import { TxMissingSummary } from './ui/tx-missing-summary'
import { TxSpotSection } from './ui/tx-spot-section'
import { TxTransferSection } from './ui/tx-transfer-section'

/**
 * One operation of any kind: the kind's own fields, then what every kind
 * shares, then the summary and the balance preview. The logic lives in
 * `model/use-tx-*` (put together by `use-tx-form`), the sections in `ui/tx-*`.
 */
const props = withDefaults(defineProps<TransactionFormProps>(), {
  transaction: null,
  plan: null,
  disabled: false,
  counterpart: null,
  template: false,
  flowId: null,
  prefill: null,
})

/** `saved` after every successful write; `submit` instead in template mode. */
const emit = defineEmits<{ saved: [tx: Transaction]; submit: [] }>()

const { form, check, save, valid, showFills, debouncedDraft, preview } = provideTxForm(props, (tx) => emit('saved', tx))

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
})
</script>

<template>
  <form
    :id="props.template ? 'transaction-template-form' : 'transaction-form'"
    class="flex flex-col gap-4"
    @submit.prevent="check.submit(submit, $event)"
  >
    <TxKindPicker />

    <TxCashSection v-if="form.kind === 'topup' || form.kind === 'payment' || form.kind === 'defi'" />
    <TxTransferSection v-else-if="form.kind === 'transfer'" />
    <TxConvertSection v-else-if="form.kind === 'convert'" />
    <TxSpotSection v-else-if="form.kind === 'trade_spot'" />
    <TxFuturesSection v-else-if="form.kind === 'trade_futures'" />
    <TxAssetSellSection v-else-if="form.kind === 'asset_sell'" />

    <TxFillsList v-if="showFills" />
    <TxCommonFields />
    <TxMissingSummary />

    <TxPreviewCard v-if="debouncedDraft" :preview="preview.data.value" :loading="preview.isFetching.value" />
  </form>
</template>
