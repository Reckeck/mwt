<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { CarryOverSwitch } from '@/features/month-carry-over'
import { Money } from '@/entities/currency'
import { useMonthBalance } from '@/entities/report'
import { SectionState } from '@/shared/ui/section-state'

import { monthBalanceFilter } from '../../lib/month-filter'

/** How the month went: what came in, what went out, what is left and, with the carry on, what is carried. */
const props = defineProps<{ month: string }>()
const { t } = useI18n()

const balance = useMonthBalance(() => monthBalanceFilter(props.month))
const row = computed(() => balance.data.value?.rows.find((entry) => entry.month === props.month) ?? null)
const carries = computed(() => balance.data.value?.carry_since != null)
const lines = computed(() => {
  const current = row.value
  if (!current) return []
  return [
    { key: 'income', label: t('balance.income'), amount: current.income, signed: true },
    { key: 'spent', label: t('balance.spent'), amount: current.spent, signed: false },
    { key: 'left', label: t('balance.left'), amount: current.left, signed: true, strong: true },
    ...(carries.value ? [{ key: 'total', label: t('balance.carried'), amount: current.total, signed: false, strong: true }] : []),
  ]
})
</script>

<template>
  <SectionState :loading="balance.isPending.value" :error="balance.error.value" skeleton="figure" compact @retry="balance.refetch()">
    <section v-if="balance.data.value" class="grid gap-3 rounded-lg border border-border bg-card p-4" data-tour="balance-months">
      <h2 class="heading-section">{{ t('money.month.balanceTitle') }}</h2>
      <dl v-if="row" class="grid grid-cols-[1fr_auto] gap-x-4 gap-y-1.5 text-sm tabular-nums">
        <template v-for="line in lines" :key="line.key">
          <dt :class="line.strong ? 'font-medium' : 'text-muted-foreground'">{{ line.label }}</dt>
          <dd :class="['text-right', line.strong && 'font-semibold', line.key === 'left' && Number(line.amount) < 0 && 'text-destructive']">
            <Money :amount="line.amount" :currency="balance.data.value.currency" :decimals="0" :signed="line.signed" />
          </dd>
        </template>
      </dl>
      <CarryOverSwitch :carry-since="balance.data.value.carry_since" :from="balance.data.value.rows[0]?.month ?? props.month" />
    </section>
  </SectionState>
</template>
