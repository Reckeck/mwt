<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { keepPreviousData, useQuery } from '@tanstack/vue-query'

import { cryptoOperationDetailKey, legAmount } from '@/entities/crypto-tx'
import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { formatDay } from '@/shared/lib/format'
import { useFormValidation } from '@/shared/lib/form'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { RadioGroup, RadioGroupItem } from '@/shared/ui/radio-group'
import { SectionState } from '@/shared/ui/section-state'

import { useLinkExchange } from '../model/use-crypto-override'

/**
 * «This is an exchange»: the operation's side of the chain. The module only suggests the fiat operations
 * that look like what paid for it (the date within a few days, the sum by the rate of the day) and the
 * user chooses; nothing is tied until «Link» is pressed. The sums are not touched.
 */
const props = defineProps<{ row: CryptoOpRow }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const picked = ref('')
const search = ref('')
watch(open, (isOpen) => {
  if (isOpen) {
    picked.value = ''
    search.value = ''
  }
})

const detail = useQuery({
  queryKey: computed(() => cryptoOperationDetailKey(props.row.id, true)),
  queryFn: () => call<CryptoOperationDetail>('crypto_operation_detail', { input: { id: props.row.id, with_candidates: true } }),
  enabled: open,
  placeholderData: keepPreviousData,
})

const candidates = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return (detail.data.value?.candidates ?? []).filter(
    (candidate) =>
      !needle || [candidate.place, candidate.comment, candidate.currency, candidate.amount, formatDay(candidate.time)].join(' ').toLowerCase().includes(needle),
  )
})
/** The best guess is picked first, so the common case is one press. */
watch(candidates, (list) => {
  if (!picked.value && list[0]?.reasons.includes('amount_fits')) picked.value = list[0].transaction_id
})

const form = useFormValidation(() => ({ picked: picked.value ? null : t('cryptoTrack.operations.exchangeDialog.pickOne') }), { resetOn: open })
const link = useLinkExchange(() => {
  open.value = false
})
const leg = computed(() => props.row.legs[0])

function reasonLine(candidate: CryptoLinkCandidate): string {
  if (candidate.reasons.length) return t('cryptoTrack.operations.reason.looksLike', { why: reasonText(candidate) })
  return [formatDay(candidate.time), candidate.comment].filter(Boolean).join(' · ')
}
function reasonText(candidate: CryptoLinkCandidate): string {
  const parts: string[] = []
  for (const reason of candidate.reasons) {
    if (reason === 'same_day') parts.push(t('cryptoTrack.operations.reason.sameDay'))
    else if (reason === 'amount_fits') {
      parts.push(
        candidate.rate && candidate.rate_symbol
          ? t('cryptoTrack.operations.reason.amountFitsRate', { rate: candidate.rate, currency: candidate.currency, symbol: candidate.rate_symbol })
          : t('cryptoTrack.operations.reason.amountFits'),
      )
    } else parts.push(t('cryptoTrack.operations.reason.nearDate', { days: reason.near_date.days }))
  }
  return parts.join(', ')
}
</script>

<template>
  <FormDialog
    v-model:open="open"
    form-id="crypto-exchange-form"
    overlay="crypto-exchange"
    size="lg"
    :title="t('cryptoTrack.operations.exchangeDialog.title')"
    :description="t('cryptoTrack.operations.exchangeDialog.hint')"
    :submit-label="t('cryptoTrack.operations.exchangeDialog.link')"
    :pending="link.isPending.value"
    phone-submit-only
    @submit="form.submit(() => link.mutate({ id: props.row.id, transactionId: picked }), $event)"
  >
    <div v-if="leg" class="rounded-lg bg-muted p-3">
      <p class="text-lg font-semibold tabular-nums"><Money :amount="legAmount(leg)" :currency="leg.symbol" signed :convert="false" /></p>
      <p class="text-xs text-muted-foreground">{{ formatDay(props.row.time) }} · {{ props.row.account_name }}</p>
    </div>

    <FormField :label="t('cryptoTrack.operations.exchangeDialog.paidWith')" for="crypto-exchange-search" required :error="form.error('picked')">
      <Input id="crypto-exchange-search" v-model="search" :placeholder="t('cryptoTrack.operations.exchangeDialog.search')" />
    </FormField>

    <SectionState
      :loading="detail.isPending.value"
      :error="detail.error.value"
      :empty="!candidates.length"
      :empty-title="t('cryptoTrack.operations.exchangeDialog.none')"
      :empty-description="t('cryptoTrack.operations.exchangeDialog.noneHint')"
      compact
      @retry="detail.refetch()"
    >
      <RadioGroup v-model="picked" class="max-h-72 gap-0 divide-y divide-border overflow-y-auto rounded-lg border" :data-invalid="form.invalidAttr('picked')">
        <div
          v-for="candidate in candidates"
          :key="candidate.transaction_id"
          class="flex items-start gap-3 px-3 py-2 hover:bg-accent"
          :class="picked === candidate.transaction_id && 'bg-primary/10'"
        >
          <RadioGroupItem :id="`crypto-candidate-${candidate.transaction_id}`" :value="candidate.transaction_id" class="mt-1" />
          <label :for="`crypto-candidate-${candidate.transaction_id}`" class="min-w-0 flex-1 cursor-pointer text-sm">
            <span class="block">
              {{ candidate.place }} ·
              <Money :amount="candidate.amount" :currency="candidate.currency" signed />
            </span>
            <span class="block text-xs" :class="candidate.reasons.length ? 'text-success' : 'text-muted-foreground'">
              {{ reasonLine(candidate) }}
            </span>
          </label>
        </div>
      </RadioGroup>
    </SectionState>
    <p class="text-xs text-muted-foreground">{{ t('cryptoTrack.operations.exchangeDialog.cashNote') }}</p>
  </FormDialog>
</template>
