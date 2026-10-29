<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ArrowRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { sign, useSharedEvents } from '@/entities/shared-event'
import { formatDateRange } from '@/shared/lib/date-range'
import { pluralForm } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'
import { useWidgetSize } from '@/shared/ui/widget-frame'

import { openEvents, totalsByCurrency } from '../lib/open-events'

/**
 * «Вместе» on the dashboard: how much the open events owe me, and what I owe, per currency.
 * S is the figure; M and L add the events themselves. With no open event it only says so.
 */
const { t, locale } = useI18n()
const router = useRouter()
const size = useWidgetSize()

const events = useSharedEvents(false)
const open = computed(() => openEvents(events.data.value ?? []))
const totals = computed(() => totalsByCurrency(open.value))
const listed = computed(() => open.value.slice(0, size.value === 'l' ? 5 : 3))

/** The list of events once its screen exists; until then the first open event. */
const target = computed(() =>
  router.hasRoute('money-together')
    ? { name: 'money-together' }
    : open.value[0]
      ? { name: 'money-together-event', params: { id: open.value[0].id } }
      : { name: 'money' },
)
</script>

<template>
  <Card size="sm">
    <SectionState
      :loading="events.isPending.value"
      :error="events.error.value"
      skeleton="figure"
      compact
      @retry="events.refetch()"
    >
      <div class="grid gap-2" data-tour="dash-shared-events">
        <template v-if="open.length">
          <div v-for="total in totals" :key="total.currency" class="grid gap-0.5">
            <p v-if="sign(total.owedToMe) > 0" class="flex flex-wrap items-baseline gap-x-2">
              <span class="text-sm text-muted-foreground">{{ t('dashboard.sharedEvents.owedToMe') }}</span>
              <span class="figure text-success"><Money :amount="total.owedToMe" :currency="total.currency" :decimals="0" :convert="false" /></span>
            </p>
            <p v-if="sign(total.iOwe) > 0" class="flex flex-wrap items-baseline gap-x-2 text-sm">
              <span class="text-muted-foreground">{{ t('dashboard.sharedEvents.iOwe') }}</span>
              <span class="font-semibold text-destructive"><Money :amount="total.iOwe" :currency="total.currency" :decimals="0" :convert="false" /></span>
            </p>
          </div>
          <p v-if="size === 's'" class="truncate text-xs text-muted-foreground">
            {{ t(`dashboard.sharedEvents.count.${pluralForm(open.length, locale)}`, { count: open.length }) }}
          </p>
          <ul v-else class="grid gap-1 text-xs">
            <li v-for="event in listed" :key="event.id">
              <RouterLink
                :to="{ name: 'money-together-event', params: { id: event.id } }"
                class="flex min-h-6 min-w-0 items-baseline justify-between gap-2 rounded-sm hover:underline focus-visible:outline-2 focus-visible:outline-ring"
              >
                <span class="truncate">
                  {{ event.title }}
                  <span class="text-muted-foreground">{{ formatDateRange({ start: event.starts_on, end: event.ends_on }, locale) }}</span>
                </span>
                <span class="shrink-0 tabular-nums" :class="sign(event.owed_to_me) > 0 ? 'text-success' : 'text-destructive'">
                  <Money
                    :amount="sign(event.owed_to_me) > 0 ? event.owed_to_me : event.i_owe"
                    :currency="event.currency"
                    :decimals="0"
                    :convert="false"
                  />
                </span>
              </RouterLink>
            </li>
          </ul>
          <Button v-if="size !== 's'" variant="link" size="sm" class="h-auto self-start p-0" as-child>
            <RouterLink :to="target">
              {{ t('dashboard.sharedEvents.open') }}
              <ArrowRight class="size-3.5" />
            </RouterLink>
          </Button>
        </template>
        <p v-else class="text-sm text-muted-foreground">{{ t('dashboard.sharedEvents.none') }}</p>
      </div>
    </SectionState>
  </Card>
</template>
