<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { DebtTermsPopover } from '@/features/debt-manage'
import { Money } from '@/entities/currency'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { useContactSummaries } from '@/shared/api/queries'
import { formatDayMonthShort, pluralForm } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import type { ContactNet } from '@/entities/contact'

/** Three figures of the card: the debt (each currency, with its term), the turnover, the last operation. */
const props = defineProps<{ contactId: string; nets: ContactNet[] }>()

const { t, locale } = useI18n()
const summaries = useContactSummaries()
const summary = computed(() => summaries.data.value?.find((row) => row.contact_id === props.contactId) ?? null)
const turnover = useQuery({
  // no placeholder: another contact's turnover
  queryKey: computed(() => ['contacts', 'turnover', props.contactId]),
  queryFn: () => call<ContactTurnover>('get_contact_turnover', { input: { contact_id: props.contactId, account_id: null } }),
})
const turnoverBase = computed(() =>
  turnover.data.value ? Number(turnover.data.value.sent_base) + Number(turnover.data.value.received_base) : null,
)
const operations = computed(() => t(`contacts.opsCount.${pluralForm(summary.value?.tx_count ?? 0, locale.value)}`, { count: summary.value?.tx_count ?? 0 }))

const CARD = 'grid min-w-0 content-start gap-1'
</script>

<template>
  <div class="grid grid-cols-3 gap-2" data-tour="contact-debts">
    <Card size="sm" class="min-h-24 min-w-0"><div :class="CARD">
      <p class="text-xs text-muted-foreground">{{ t('contacts.debt.label') }}</p>
      <template v-if="props.nets.length">
        <p
          v-for="(row, index) in props.nets"
          :key="row.symbol"
          :class="cn('truncate tabular-nums', index ? 'text-sm' : 'text-xl font-semibold', row.net > 0 ? 'text-success' : 'text-destructive')"
        >
          <Money :amount="row.net" :currency="row.symbol" signed :decimals="0" :convert="false" />
        </p>
        <p class="text-xs text-muted-foreground">
          <DebtTermsPopover :contact-id="props.contactId" :asset="props.nets[0].symbol" />
        </p>
      </template>
      <p v-else class="text-base font-medium text-muted-foreground">{{ t('contacts.debt.none') }}</p>
    </div></Card>
    <Card size="sm" class="min-w-0"><div :class="CARD">
      <p class="text-xs text-muted-foreground">{{ t('contacts.sheet.turnover') }}</p>
      <!-- Operations the rates do not price (exchanges) add up to nothing: then only their count is the figure. -->
      <template v-if="turnoverBase">
        <p class="truncate text-xl font-semibold tabular-nums"><Money :amount="turnoverBase" :decimals="0" :convert="false" /></p>
        <p class="text-xs text-muted-foreground">{{ operations }}</p>
      </template>
      <p v-else class="truncate text-xl font-semibold">{{ operations }}</p>
    </div></Card>
    <Card size="sm" class="min-w-0"><div :class="CARD">
      <p class="text-xs text-muted-foreground">{{ t('contacts.sheet.last') }}</p>
      <p class="truncate text-xl font-semibold">{{ formatDayMonthShort(summary?.last_tx_at) }}</p>
      <p class="truncate text-xs tabular-nums text-muted-foreground">
        <Money v-if="summary?.last_amount && summary.last_asset" :amount="summary.last_amount" :currency="summary.last_asset" signed :convert="false" />
      </p>
    </div></Card>
  </div>
</template>
