<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { EVENT_STATUS_VARIANT, eventStatusKey, sign } from '@/entities/shared-event'
import { formatDateRange } from '@/shared/lib/date-range'
import { Badge } from '@/shared/ui/badge'
import { Card } from '@/shared/ui/card'

/** A row of the events list: the title and dates, who leads and how many people, the status and the debt with me. */
const props = defineProps<{ event: SharedEventSummary }>()
const { t, locale } = useI18n()

const dates = computed(() => formatDateRange({ start: props.event.starts_on, end: props.event.ends_on }, locale.value))
const owed = computed(() => sign(props.event.owed_to_me) > 0)
const owe = computed(() => sign(props.event.i_owe) > 0)
const led = computed(() => (props.event.organizer_is_me ? t('sharedEvents.list.ledByMe') : t('sharedEvents.list.ledBy', { name: props.event.organizer_name })))
</script>

<template>
  <Card class="p-0">
    <RouterLink
      :to="{ name: 'money-together-event', params: { id: props.event.id } }"
      class="touch-hit flex min-h-16 min-w-0 items-center gap-3 rounded-xl px-4 py-3 hover:bg-accent focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
    >
      <div class="grid min-w-0 flex-1 gap-1">
        <div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1">
          <span class="truncate font-medium">{{ props.event.title }}</span>
          <span v-if="dates" class="shrink-0 text-sm text-muted-foreground">{{ dates }}</span>
          <Badge :variant="EVENT_STATUS_VARIANT[props.event.status]">{{ t(eventStatusKey(props.event.status)) }}</Badge>
          <Badge v-if="props.event.archived" variant="outline">{{ t('sharedEvents.archivedBadge') }}</Badge>
        </div>
        <p class="truncate text-xs text-muted-foreground">
          {{ t('sharedEvents.list.people', { count: props.event.people_count }) }} · {{ led }}
        </p>
      </div>
      <div class="grid shrink-0 justify-items-end gap-0.5 text-sm tabular-nums">
        <span v-if="owed" class="font-semibold text-success">
          {{ t('sharedEvents.list.owedToMe') }}
          <Money :amount="props.event.owed_to_me" :currency="props.event.currency" :decimals="0" :convert="false" />
        </span>
        <span v-else-if="owe" class="font-semibold text-destructive">
          {{ t('sharedEvents.list.iOwe') }}
          <Money :amount="props.event.i_owe" :currency="props.event.currency" :decimals="0" :convert="false" />
        </span>
        <span v-else class="text-xs text-muted-foreground">
          <Money :amount="props.event.spent" :currency="props.event.currency" :decimals="0" :convert="false" />
        </span>
      </div>
      <ChevronRight class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
    </RouterLink>
  </Card>
</template>
