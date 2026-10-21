<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Bell, BellRing, CalendarClock } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { dateFormat } from '@/shared/lib/format'
import { noticeRoute } from '@/shared/lib/notice-link'

/**
 * The chosen day's money and messages: spent, the month's rest after it and
 * what each following day may spend (the spreadsheet's running balance), then
 * the notifications and price alerts of the day and what falls due on it.
 */
const props = defineProps<{ day: CalendarDay; currency: string; upcoming: UpcomingItem[] }>()
const { t } = useI18n()
const route = useRoute()
const linkOf = (link: string | null) => noticeRoute(link, route.query)

const time = (iso: string) => dateFormat({ hour: '2-digit', minute: '2-digit', hourCycle: 'h23' }).format(new Date(iso))
const hasBalance = computed(() => props.day.left !== null)
</script>

<template>
  <div class="grid gap-3">
    <dl v-if="hasBalance || Number(props.day.daily)" class="grid grid-cols-[minmax(0,1fr)_auto] gap-x-4 gap-y-1 text-sm">
      <template v-if="!props.day.forecast">
        <dt class="text-muted-foreground">{{ t('calendar.spentDay') }}</dt>
        <dd class="text-right tabular-nums"><Money :amount="props.day.daily" :currency="props.currency" /></dd>
      </template>
      <template v-if="hasBalance">
        <dt class="text-muted-foreground">{{ props.day.forecast ? t('calendar.leftForecast') : t('calendar.leftMonth') }}</dt>
        <dd class="text-right tabular-nums" :class="Number(props.day.left) < 0 && 'text-destructive'">
          <Money :amount="props.day.left" :currency="props.currency" />
        </dd>
      </template>
      <template v-if="props.day.per_day_left !== null">
        <dt class="text-muted-foreground">{{ t('calendar.perDayLeftLabel') }}</dt>
        <dd class="text-right tabular-nums" :class="Number(props.day.per_day_left) < 0 && 'text-destructive'">
          <Money :amount="props.day.per_day_left" :currency="props.currency" />
        </dd>
      </template>
    </dl>

    <ul v-if="props.day.notices.length || props.upcoming.length" class="grid gap-1.5 text-sm">
      <li v-for="notice in props.day.notices" :key="notice.key" class="flex min-w-0 items-center gap-2">
        <BellRing v-if="notice.category === 'price_alert'" class="size-4 shrink-0 text-warning" />
        <Bell v-else class="size-4 shrink-0 text-muted-foreground" />
        <span class="w-10 shrink-0 text-xs tabular-nums text-muted-foreground">{{ time(notice.at) }}</span>
        <RouterLink v-if="linkOf(notice.link)" :to="linkOf(notice.link) ?? ''" class="min-w-0 truncate hover:underline">{{ notice.title }}</RouterLink>
        <span v-else class="min-w-0 truncate">{{ notice.title }}</span>
      </li>
      <li v-for="item in props.upcoming" :key="`${item.kind}-${item.source_id}-${item.at}`" class="flex min-w-0 items-center gap-2">
        <CalendarClock class="size-4 shrink-0 text-muted-foreground" />
        <span class="w-10 shrink-0 text-xs tabular-nums text-muted-foreground">{{ time(item.at) }}</span>
        <span class="min-w-0 flex-1 truncate">{{ item.title }}</span>
        <span v-if="item.amount" class="shrink-0 tabular-nums text-muted-foreground">
          <template v-if="item.estimated">~</template><Money :amount="item.amount" :currency="item.asset?.symbol ?? props.currency" />
        </span>
      </li>
    </ul>
  </div>
</template>
