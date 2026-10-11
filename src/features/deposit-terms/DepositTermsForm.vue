<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { Plus, Trash2 } from 'lucide-vue-next'
import { toast } from 'vue-sonner'

import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { Select } from '@/shared/ui/select'
import { call, parseError } from '@/shared/api/ipc'
import { localDate } from '@/shared/lib/planning'
import { STORE_TREE_QUERY_KEY } from '@/shared/lib/store-tree'
import { codeErrorMessage } from '@/shared/lib/vault-error'

import { CAPITALIZATIONS, INCOME_BRACKETS } from './deposit-terms.constants'
import type { DepositTermsFormProps } from './deposit-terms.types'

/**
 * The terms of a savings account, deposit or ПДС. Rates are a history: a new
 * rate is a new row from its date, so interest already earned at the old rate
 * stays as it was.
 */
const props = defineProps<DepositTermsFormProps>()
const { t } = useI18n()
const queryClient = useQueryClient()

const isDeposit = computed(() => props.store.sub_type === 'deposit')
const isPds = computed(() => props.store.sub_type === 'pds')

interface RateRow {
  from: string
  rate: string
}
const rates = ref<RateRow[]>([])
const capitalization = ref<Capitalization>('monthly')
const openedAt = ref('')
const termMonths = ref('')
const autoAccrue = ref(false)
const firstContribution = ref('')
const bracket = ref<IncomeBracket>('low')

watch(
  () => props.store,
  (store) => {
    const terms = store.deposit
    rates.value = (terms?.rates ?? []).map((period) => ({ from: period.from, rate: period.rate }))
    if (!rates.value.length) rates.value = [{ from: localDate(new Date()), rate: '' }]
    capitalization.value = terms?.capitalization ?? 'monthly'
    openedAt.value = terms?.opened_at ?? ''
    termMonths.value = terms?.term_months ? String(terms.term_months) : ''
    autoAccrue.value = terms?.auto_accrue ?? false
    firstContribution.value = terms?.pds?.first_contribution_at ?? ''
    bracket.value = terms?.pds?.income_bracket ?? 'low'
  },
  { immediate: true },
)

/** The key rate is a yardstick for the bank's offer, and the tax limit hangs on it. */
const tax = useQuery({ queryKey: ['interest-tax'], queryFn: () => call<InterestTaxReport>('get_interest_tax', { year: null }) })

function addRate() {
  rates.value = [...rates.value, { from: localDate(new Date()), rate: rates.value.at(-1)?.rate ?? '' }]
}

const valid = computed(() => rates.value.every((row) => row.from && row.rate !== '') && (!isPds.value || !!firstContribution.value))

const save = useMutation({
  mutationFn: () =>
    call<Store>('set_deposit_terms', {
      input: {
        store_id: props.store.id,
        terms: {
          rates: rates.value.map((row) => ({ from: row.from, rate: row.rate })),
          capitalization: capitalization.value,
          opened_at: isDeposit.value && openedAt.value ? openedAt.value : null,
          term_months: isDeposit.value && termMonths.value ? Number(termMonths.value) : null,
          auto_accrue: autoAccrue.value,
          pds: isPds.value ? { first_contribution_at: firstContribution.value, income_bracket: bracket.value } : null,
        },
      } satisfies SetDepositTermsInput,
    }),
  onSuccess: () => {
    toast.success(t('settings.saved'))
    for (const queryKey of [STORE_TREE_QUERY_KEY, ['store'], ['deposit-report']]) void queryClient.invalidateQueries({ queryKey })
  },
  onError: (err) => toast.error(t('settings.saveFailed'), { description: codeErrorMessage(parseError(err), t) }),
})
</script>

<template>
  <form class="@container grid gap-4" @submit.prevent="valid && save.mutate()">
    <div class="grid gap-2">
      <div class="flex items-center justify-between gap-2">
        <Label>{{ t('deposit.rates') }}</Label>
        <span v-if="tax.data.value?.max_key_rate" class="text-xs text-muted-foreground">
          {{ t('deposit.keyRate', { rate: tax.data.value.max_key_rate }) }}
        </span>
      </div>
      <div v-for="(row, index) in rates" :key="index" class="flex items-center gap-2">
        <Input v-model="row.from" type="date" class="w-40" :aria-label="t('deposit.rateFrom')" />
        <AmountInput v-model="row.rate" :decimals="3" class="w-32" :aria-label="t('deposit.rate')">
          <template #suffix><span class="px-2 text-xs text-muted-foreground">%</span></template>
        </AmountInput>
        <Button
          v-if="rates.length > 1"
          type="button"
          variant="ghost"
          size="icon"
          class="size-8 text-muted-foreground"
          :aria-label="t('deposit.removeRate')"
          @click="rates.splice(index, 1)"
        >
          <Trash2 class="size-4" />
        </Button>
      </div>
      <Button type="button" variant="outline" size="sm" class="w-fit" @click="addRate">
        <Plus class="size-4" />
        {{ t('deposit.addRate') }}
      </Button>
      <p class="text-xs text-muted-foreground">{{ t('deposit.ratesHint') }}</p>
    </div>

    <div class="grid gap-4 @sm:grid-cols-3">
      <div class="grid gap-1.5">
        <Label for="deposit-cap">{{ t('deposit.capitalization') }}</Label>
        <Select id="deposit-cap" v-model="capitalization">
          <option v-for="entry in CAPITALIZATIONS" :key="entry" :value="entry">{{ t(`deposit.cap.${entry}`) }}</option>
        </Select>
      </div>
      <template v-if="isDeposit">
        <div class="grid gap-1.5">
          <Label for="deposit-opened">{{ t('deposit.openedAt') }}</Label>
          <Input id="deposit-opened" v-model="openedAt" type="date" />
        </div>
        <div class="grid gap-1.5">
          <Label for="deposit-term">{{ t('deposit.termMonths') }}</Label>
          <Input id="deposit-term" v-model="termMonths" type="number" min="1" max="120" />
        </div>
      </template>
    </div>

    <label class="flex items-center gap-2 text-sm">
      <input v-model="autoAccrue" type="checkbox" class="size-4 accent-primary" />
      {{ t('deposit.autoAccrue') }}
    </label>

    <div v-if="isPds" class="grid gap-3 rounded-lg border border-border p-3">
      <p class="text-sm font-medium">{{ t('deposit.pds.title') }}</p>
      <p class="text-xs text-muted-foreground">{{ t('deposit.pds.hint') }}</p>
      <div class="grid gap-4 @sm:grid-cols-2">
        <div class="grid gap-1.5">
          <Label for="pds-first">{{ t('deposit.pds.firstContribution') }}</Label>
          <Input id="pds-first" v-model="firstContribution" type="date" />
        </div>
        <div class="grid gap-1.5">
          <Label for="pds-bracket">{{ t('deposit.pds.bracket') }}</Label>
          <Select id="pds-bracket" v-model="bracket">
            <option v-for="entry in INCOME_BRACKETS" :key="entry" :value="entry">{{ t(`deposit.pds.brackets.${entry}`) }}</option>
          </Select>
        </div>
      </div>
    </div>

    <div>
      <Button type="submit" :disabled="!valid || save.isPending.value">{{ t('actions.save') }}</Button>
    </div>
  </form>
</template>
