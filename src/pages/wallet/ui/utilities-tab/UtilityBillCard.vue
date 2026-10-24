<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { UtilityRemindersButton } from '@/features/utility-reminders'
import { Button } from '@/shared/ui/button'
import { formatDayMonthShort, formatMoney, formatMonthKey, pluralForm } from '@/shared/lib/format'

/**
 * «Счёт за октябрь · оплатить до 10 нояб» (Wallets-2 · 10): what the month will
 * cost, why it is still an estimate, the rent an automation pays apart, and how
 * it compares with the last six months.
 */
const props = defineProps<{ store: Store; bill: UtilityBill; phone: boolean }>()
const emit = defineEmits<{ pay: []; readings: [] }>()
const { t, locale } = useI18n()

const currency = computed(() => props.bill.lines[0]?.item.currency ?? props.store.property?.currency ?? 'rub')
const title = computed(() =>
  t('utilityTab.billTitle', {
    month: formatMonthKey(props.bill.period, { month: 'long' }),
    due: formatDayMonthShort(props.bill.due_at),
  }),
)
const unread = computed(() =>
  props.bill.unread ? t(`utilityTab.unread.${pluralForm(props.bill.unread, locale.value)}`, { count: props.bill.unread }) : '',
)
const automated = computed(() =>
  props.bill.lines.filter((line) => 'housing' in line.item.kind && line.item.kind.housing.automation_id),
)
const automatedNote = computed(() =>
  automated.value.map((line) =>
    t('utilityTab.automatedNote', {
      name: line.item.name,
      amount: formatMoney(line.amount, { currency: line.item.currency, decimals: 0 }),
    }),
  ),
)
/** The phone's one line: «1 статья ждёт показаний · аренду платит автоматизация». */
const phoneNote = computed(() =>
  [unread.value, ...(automated.value.length ? [t('utilityTab.automatedShort')] : [])].filter(Boolean).join(' · '),
)
const delta = computed(() => (props.bill.delta_pct === null ? null : Number(props.bill.delta_pct)))
</script>

<template>
  <div class="flex flex-col gap-2.5 rounded-xl border border-border bg-card p-4 shadow-sm max-md:gap-2" data-utility-bill>
    <div class="flex flex-wrap items-end gap-x-5 gap-y-3">
      <div class="flex min-w-0 flex-col gap-0.5">
        <span class="text-sm text-muted-foreground">{{ title }}</span>
        <span class="text-xl font-semibold tabular-nums max-md:text-[1.75rem] max-md:leading-tight">
          <template v-if="props.bill.estimated">≈ </template><Money :amount="props.bill.total" :currency="currency" :decimals="0" />
        </span>
        <span v-if="props.phone && phoneNote" class="mt-1.5 text-xs text-warning">{{ phoneNote }}</span>
      </div>
      <div v-if="!props.phone" class="ml-auto flex items-center gap-2">
        <UtilityRemindersButton :store="props.store" />
        <Button variant="outline" :disabled="props.store.archived" @click="emit('readings')">{{ t('utilityTab.readings') }}</Button>
        <Button :disabled="props.store.archived" data-tour="utility-pay" data-bill-pay @click="emit('pay')">{{ t('utilityBill.open') }}</Button>
      </div>
    </div>
    <div v-if="!props.phone" class="flex flex-wrap items-center gap-x-4 gap-y-1 text-xs">
      <span v-if="unread" class="text-warning">{{ t('utilityTab.unreadHint', { unread }) }}</span>
      <span v-for="note in automatedNote" :key="note" class="text-muted-foreground">{{ note }}</span>
      <span v-if="props.bill.average" class="ml-auto whitespace-nowrap text-muted-foreground">
        {{ t('utilityTab.average', { amount: formatMoney(props.bill.average, { currency, decimals: 0 }) }) }}
        <template v-if="delta !== null">
          · <span :class="delta < 0 ? 'text-success' : delta > 0 ? 'text-warning' : ''">{{ delta > 0 ? '+' : delta < 0 ? '−' : '' }}{{ Math.abs(delta) }} %</span>
        </template>
      </span>
    </div>
    <div v-if="props.phone" class="grid grid-cols-2 gap-2">
      <Button variant="outline" class="h-11" :disabled="props.store.archived" @click="emit('readings')">{{ t('utilityTab.readingsShort') }}</Button>
      <Button class="h-11" :disabled="props.store.archived" data-bill-pay @click="emit('pay')">{{ t('utilityBill.open') }}</Button>
    </div>
  </div>
</template>
