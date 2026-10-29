<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { EVENT_STATUS_VARIANT, eventStatusKey, sign, useContactSharedEvents } from '@/entities/shared-event'
import { formatDateRange } from '@/shared/lib/date-range'
import { formatDayMonthShort } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'
import { Skeleton } from '@/shared/ui/skeleton'


/**
 * «Общие события» in a contact's card: the events I share with them, each with the balance
 * between us (green: they owe me, red: I owe them). Without events it says so in a row of the same height.
 */
const props = defineProps<{ contactId: string }>()

const ROW_CLASS = [
  'flex min-h-[3.25rem] flex-wrap items-center gap-x-3 gap-y-1 px-4 py-3 text-sm hover:bg-muted/50',
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-inset focus-visible:ring-ring',
]

const { t, locale } = useI18n()
const events = useContactSharedEvents(() => props.contactId)
const rows = computed(() =>
  (events.data.value ?? []).map((row) => ({
    ...row,
    dates: formatDateRange({ start: row.starts_on, end: row.ends_on }, locale.value),
    direction: sign(row.amount),
    owed: row.amount.replace('-', ''),
    decimals: /^-?\d+(\.0+)?$/.test(row.amount) ? 0 : 2,
  })),
)
</script>

<template>
  <section class="grid gap-2" data-tour="contact-shared-events">
    <h3 class="text-base font-semibold">{{ t('contacts.sharedEvents.title') }}</h3>
    <!-- One row tall while loading and while empty, so the card below does not move when the events arrive. -->
    <div class="min-h-14 divide-y divide-border overflow-hidden rounded-xl border border-border bg-card shadow-sm">
      <Skeleton v-if="events.isPending.value" class="h-[3.25rem] w-full rounded-none" />
      <p v-else-if="!rows.length" class="flex min-h-[3.25rem] items-center px-4 text-sm text-muted-foreground">
        {{ t('contacts.sharedEvents.none') }}
      </p>
      <RouterLink
        v-for="row in rows"
        :key="row.event_id"
        :to="{ name: 'money-together-event', params: { id: row.event_id } }"
        :class="ROW_CLASS"
      >
        <span class="min-w-0 truncate font-medium">
          {{ row.title }}<template v-if="row.dates"> · {{ row.dates }}</template>
        </span>
        <Badge :variant="EVENT_STATUS_VARIANT[row.status]">{{ t(eventStatusKey(row.status)) }}</Badge>
        <span class="ml-auto flex items-baseline gap-2 tabular-nums">
          <span v-if="row.due_at" class="text-xs text-muted-foreground">
            {{ t('sharedEvents.due.until', { date: formatDayMonthShort(row.due_at) }) }}
          </span>
          <i18n-t
            v-if="row.direction !== 0"
            :keypath="row.direction > 0 ? 'contacts.sharedEvents.owesMe' : 'contacts.sharedEvents.iOwe'"
            tag="span"
            :class="row.direction > 0 ? 'font-semibold text-success' : 'font-semibold text-destructive'"
          >
            <template #amount><Money :amount="row.owed" :currency="row.currency" :decimals="row.decimals" :convert="false" /></template>
          </i18n-t>
          <span v-else class="text-muted-foreground">{{ t('contacts.sharedEvents.even') }}</span>
        </span>
      </RouterLink>
    </div>
  </section>
</template>
