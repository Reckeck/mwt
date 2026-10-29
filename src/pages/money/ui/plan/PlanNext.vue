<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { Money } from '@/entities/currency'
import { call } from '@/shared/api/ipc'
import { formatDate } from '@/shared/lib/format'
import { Badge } from '@/shared/ui/badge'

/** `Later this month` (Money 2 · 9, 10): what the automations and reminders bring before the month ends. */
const { t } = useI18n()

const now = new Date()
const days = Math.max(1, new Date(now.getFullYear(), now.getMonth() + 1, 0).getDate() - now.getDate() + 1)
const upcoming = useQuery({ queryKey: ['upcoming', days], queryFn: () => call<UpcomingItem[]>('get_upcoming', { days }) })
const items = computed(() => (upcoming.data.value ?? []).slice(0, 6))

const KIND_LABEL: Partial<Record<UpcomingKind, string>> = { automation: 'automation', reminder: 'reminder' }
const kindText = (kind: UpcomingKind) => t(`money.plan.kind.${KIND_LABEL[kind] ?? 'other'}`)
</script>

<template>
  <section v-if="items.length" class="grid gap-3 rounded-xl border bg-card p-4 shadow-sm" data-tour="plan-next">
    <header class="flex items-baseline justify-between gap-2">
      <h2 class="heading-section">{{ t('money.plan.next') }}</h2>
      <RouterLink :to="{ name: 'automations' }" class="text-xs text-muted-foreground hover:text-foreground">{{ t('money.plan.fromAutomations') }}</RouterLink>
    </header>
    <ul class="grid gap-2">
      <li v-for="item in items" :key="`${item.kind}-${item.source_id}-${item.at}`" class="flex min-w-0 items-center gap-2 text-sm">
        <Badge variant="secondary" class="shrink-0">{{ kindText(item.kind) }}</Badge>
        <span class="min-w-0 truncate">
          {{ formatDate(item.date) }} · {{ item.title }}
          <b v-if="item.amount" class="tabular-nums font-medium">
            <Money :amount="item.amount" :currency="item.asset?.symbol" :decimals="0" :at="item.at" />
          </b>
        </span>
      </li>
    </ul>
  </section>
</template>
