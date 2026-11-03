<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'
import { pluralForm } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import type { ContactDirection, ContactTotals } from '../contact-table.types'

/**
 * «They owe me / I owe» in the base currency: the page's first answer. A tile is also a filter:
 * a click lists only the contacts of that direction, a second click lifts it.
 */
const props = defineProps<{ totals: ContactTotals; loading: boolean }>()
const direction = defineModel<ContactDirection | null>('direction', { required: true })

const { t, locale } = useI18n()
const count = (value: number) => t(`contacts.table.contactCount.${pluralForm(value, locale.value)}`, { count: value })

function toggle(next: ContactDirection) {
  direction.value = direction.value === next ? null : next
}
</script>

<template>
  <div class="grid grid-cols-2 gap-3 md:flex md:flex-wrap" data-tour="contact-summary">
    <Button
      v-for="tile in [
        { key: 'receivable', label: t('contacts.table.owedToMe'), amount: props.totals.receivable, number: props.totals.receivableCount, tone: 'text-success' },
        { key: 'payable', label: t('contacts.table.iOwe'), amount: props.totals.payable, number: props.totals.payableCount, tone: 'text-destructive' },
      ] as const"
      :key="tile.key"
      variant="outline"
      :aria-pressed="direction === tile.key"
      :class="
        cn(
          'h-auto min-h-[4.5rem] min-w-0 flex-col items-start justify-between gap-1 px-4 py-3 text-left font-normal md:w-56',
          direction === tile.key && 'border-primary bg-accent',
        )
      "
      @click="toggle(tile.key)"
    >
      <span class="text-sm text-muted-foreground">{{ tile.label }}</span>
      <span class="flex w-full min-w-0 items-baseline justify-between gap-2">
        <span :class="cn('truncate text-2xl font-semibold tabular-nums', tile.tone)">
          <template v-if="props.totals.missingRate && tile.key === 'receivable'">≈ </template>
          <Money :amount="tile.amount" :convert="false" :decimals="0" />
        </span>
        <span class="shrink-0 text-xs text-muted-foreground max-md:hidden">{{ count(tile.number) }}</span>
      </span>
    </Button>
    <p v-if="props.totals.missingRate" class="col-span-2 -mt-1 text-xs text-muted-foreground md:w-full">
      {{ t('contacts.table.missingRate') }}
    </p>
  </div>
</template>
