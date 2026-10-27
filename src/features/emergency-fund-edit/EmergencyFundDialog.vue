<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { useStoreTree } from '@/shared/api/queries'
import { Money } from '@/entities/currency'
import { StoreNodeSelect } from '@/entities/store'
import { formatMoney, formatQuantity } from '@/shared/lib/format'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { AmountInput } from '@/shared/ui/amount-input'
import { FormDialog } from '@/shared/ui/form-dialog'
import { FormField } from '@/shared/ui/form-field'
import { SwitchRow } from '@/shared/ui/switch-row'

import { useFundForm } from './model/use-fund-form'

/**
 * The fund (Money 3 · 17): how many months, where it lies, the sum that makes
 * and «Создать цель «Подушка» в Плане». From the checkup and its full card.
 */
const props = defineProps<{ fund: EmergencyFund; status?: EmergencyStatus | null }>()
const open = defineModel<boolean>('open', { required: true })

const { t } = useI18n()
const tree = useStoreTree(open)
const { storeIds, targetMonths, avgMonths, makeGoal, goal, form, set, save } = useFundForm(
  () => props.fund,
  () => props.status,
  open,
)

// money: plain — the hint under the field is a string prop of FormField
const money = (amount: string, currency: string) => formatMoney(amount, { currency, decimals: 0 })
/** «Расходы в месяц: 1 265 $ — по средним за 3 месяца»; the general advice until spending is known. */
const monthsHint = computed(() => {
  const status = props.status
  if (!status?.e_mand) return t('emergency.dialog.targetHint')
  return t('emergency.dialog.spending', {
    amount: money(status.e_mand, status.currency),
    months: formatQuantity(String(status.e_mand_months || props.fund.avg_months), 0),
  })
})
</script>

<template>
  <FormDialog overlay="emergency-fund"
    cancel-variant="outline"
    v-model:open="open"
    :title="t('emergency.dialog.title')"
    form-id="emergency-fund-form"
    :submit-label="t('emergency.dialog.save')"
    :pending="set.isPending.value"
    @submit="form.submit(() => save(t('emergency.goalName')), $event)"
  >
    <FormField for="emergency-target" :label="t('emergency.dialog.target')" :hint="monthsHint" :error="form.error('target')" required>
      <AmountInput id="emergency-target" v-model="targetMonths" :decimals="0">
        <template #suffix><span class="px-2 text-xs text-muted-foreground">{{ t('emergency.dialog.monthsUnit') }}</span></template>
      </AmountInput>
    </FormField>
    <FormField
      for="emergency-accounts"
      :label="t('emergency.dialog.accounts')"
      :hint="t('emergency.dialog.accountsHint')"
      :error="form.error('accounts')"
      required
    >
      <StoreNodeSelect
        id="emergency-accounts"
        v-model:ids="storeIds"
        :nodes="tree.data.value ?? []"
        purpose="savings"
        multiple
        :placeholder="t('emergency.dialog.none')"
      />
    </FormField>
    <template v-if="goal">
      <div class="flex flex-wrap items-baseline justify-between gap-x-3 rounded-lg bg-muted/60 px-3 py-2.5 text-sm" data-fund-goal>
        <span class="text-muted-foreground">{{ t('emergency.dialog.goal') }}</span>
        <i18n-t :keypath="Number(goal.gap) > 0 ? 'emergency.dialog.goalGap' : 'emergency.dialog.goalDone'" tag="b" class="font-medium tabular-nums">
          <template #amount><Money :amount="goal.amount" :currency="goal.currency" :decimals="0" /></template>
          <template #gap><Money :amount="goal.gap" :currency="goal.currency" :decimals="0" /></template>
        </i18n-t>
      </div>
      <SwitchRow id="emergency-make-goal" v-model="makeGoal" :label="t('emergency.dialog.makeGoal')" plain />
    </template>
    <AdvancedFields :default-open="!!form.error('avg')">
      <FormField for="emergency-avg" :label="t('emergency.dialog.avg')" :hint="t('emergency.dialog.avgHint')" :error="form.error('avg')" required>
        <AmountInput id="emergency-avg" v-model="avgMonths" :decimals="0">
          <template #suffix><span class="px-2 text-xs text-muted-foreground">{{ t('emergency.dialog.monthsUnit') }}</span></template>
        </AmountInput>
      </FormField>
    </AdvancedFields>
  </FormDialog>
</template>
