<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Star } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { StoreLeafSelect } from './store-leaf-select'
import { formatMoney } from '@/shared/lib/format'
import type { FormValidation } from '@/shared/lib/form'
import { AdvancedFields } from '@/shared/ui/advanced-fields'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { DateTimePicker } from '@/shared/ui/date-time-picker'
import { FormField } from '@/shared/ui/form-field'
import { SwitchRow } from '@/shared/ui/switch-row'

import { GOAL_SAVE_PCT } from '../goal-edit.constants'
import type { GoalField, GoalRemainder } from '../goal-edit.types'

/**
 * Where the savings lie (one wallet), by when, whether this month's remainder
 * goes in at once and a share of each salary after it; what the goal will
 * show once saved. The rare terms wait under `More`.
 */
const props = defineProps<{
  form: FormValidation<GoalField>
  nodes: StoreTreeNode[]
  /** Offered for a new goal only. */
  remainder: GoalRemainder | null
  creating: boolean
  target: string
  currency: string
  /** What the goal has from the start — a calculation's starting sum. */
  already?: string | null
  /** An edited goal's money in its wallet (amount in the goal's currency): a new wallet does not move it. */
  saved?: string | null
}>()
const storeIds = defineModel<string[]>('storeIds', { required: true })
const deadline = defineModel<string>('deadline', { required: true })
/** A goal has exactly one wallet: the list holds one id. */
const storeId = computed({
  get: () => storeIds.value[0] ?? '',
  set: (id: string) => (storeIds.value = id ? [id] : []),
})
const fund = defineModel<boolean>('fund', { required: true })
const autosave = defineModel<boolean>('autosave', { required: true })
const startsAt = defineModel<string>('startsAt', { required: true })
const commitment = defineModel<string>('commitment', { required: true })
const expectedReturn = defineModel<string>('expectedReturn', { required: true })
const priority = defineModel<number>('priority', { required: true })
const active = defineModel<boolean>('active', { required: true })

const { t } = useI18n()
// money: plain — the switch's label is a string prop of SwitchRow
const money = (value: string | number) => formatMoney(value, { decimals: 0, currency: props.currency })

const offer = computed(() =>
  props.creating && props.remainder && props.remainder.currency.toLowerCase() === props.currency.toLowerCase() ? props.remainder : null,
)
/** `1,539 $ of 5,000 $ · 31 %` when the remainder goes in, or with what a calculation started from. */
const after = computed(() => {
  const goal = Number(props.target)
  const start = offer.value && fund.value ? Number(offer.value.amount) : Number(props.already ?? 0)
  if (!(start > 0) || !(goal > 0)) return null
  const saved = Math.min(start, goal)
  const pct = Math.round((saved / goal) * 100)
  return { saved, target: goal, pct }
})
const advanced = computed(() => Boolean(props.form.error('startsAt') || props.form.error('commitment') || props.form.error('expectedReturn')))
</script>

<template>
  <div class="grid gap-4">
    <FormField for="goal-stores" data-tour="goal-stores" :label="t('goal.where')" required :error="props.form.error('stores')">
      <StoreLeafSelect id="goal-stores" v-model="storeId" :nodes="props.nodes" :placeholder="t('goal.wherePick')" :invalid="Boolean(props.form.error('stores'))" />
      <p class="text-xs text-muted-foreground">
        <i18n-t v-if="props.saved" keypath="goal.whereSaved">
          <template #saved><Money :amount="props.saved" :currency="props.currency" :decimals="0" /></template>
        </i18n-t>
        <template v-else>{{ t('goal.whereHint') }}</template>
      </p>
    </FormField>

    <FormField for="goal-deadline" :label="t('goal.deadline')" :error="props.form.error('deadline')">
      <DateTimePicker id="goal-deadline" v-model="deadline" :placeholder="t('goal.noDeadline')" />
    </FormField>

    <template v-if="props.creating">
      <SwitchRow v-if="offer" id="goal-fund" plain v-model="fund" :label="t('goal.fundNow', { month: t(`period.of.${offer.month.slice(5, 7)}`), amount: money(offer.amount) })" />
      <SwitchRow id="goal-autosave" plain v-model="autosave" :label="t('goal.autosave', { pct: GOAL_SAVE_PCT })" :hint="t('goal.autosaveHint')" />
    </template>

    <p v-if="props.already && Number(props.already) > 0" class="text-sm">
      {{ t('goal.already') }} <b class="tabular-nums"><Money :amount="props.already" :currency="props.currency" :decimals="0" /></b>
      <span class="text-xs text-muted-foreground"> · {{ t('goal.alreadyHint') }}</span>
    </p>
    <div v-if="after" class="flex items-center justify-between gap-3 rounded-lg bg-muted px-3 py-2.5 text-sm">
      <span>{{ t('goal.after') }}</span>
      <i18n-t keypath="goal.afterValue" tag="b" class="tabular-nums">
        <template #saved><Money bare :amount="after.saved" :currency="props.currency" :decimals="0" /></template>
        <template #target><Money :amount="after.target" :currency="props.currency" :decimals="0" /></template>
        <template #pct>{{ after.pct }}</template>
      </i18n-t>
    </div>

    <AdvancedFields :default-open="advanced">
      <div class="grid items-start gap-3 sm:grid-cols-2">
        <FormField for="goal-start" :label="t('goal.startsAt')" required :error="props.form.error('startsAt')">
          <DateTimePicker id="goal-start" v-model="startsAt" />
        </FormField>
        <FormField for="goal-commitment" :label="t('goal.commitment')" :error="props.form.error('commitment')">
          <AmountInput id="goal-commitment" v-model="commitment" :placeholder="t('goal.optional')" />
        </FormField>
      </div>
      <FormField for="goal-return" :label="t('goal.expectedReturn')" :hint="t('goal.expectedReturnHint')" :error="props.form.error('expectedReturn')">
        <AmountInput id="goal-return" v-model="expectedReturn" :decimals="2" class="w-32" :placeholder="t('goal.optional')" />
      </FormField>
      <fieldset class="flex items-center gap-1">
        <legend class="float-left mr-2 text-sm font-medium">{{ t('goal.priority') }}</legend>
        <Button
          v-for="level in 5"
          :key="level"
          variant="ghost"
          size="icon-sm"
          :aria-pressed="priority === level"
          :aria-label="String(level)"
          @click="priority = level"
        >
          <Star class="size-5" :class="level <= priority ? 'fill-warning text-warning' : 'text-muted-foreground'" />
        </Button>
      </fieldset>
      <SwitchRow id="goal-active" v-model="active" :label="t('goal.active')" />
    </AdvancedFields>
  </div>
</template>
