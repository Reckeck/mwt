<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Bell, CalendarClock, FlaskConical, Repeat, Workflow } from 'lucide-vue-next'
import { RouterLink, useRoute } from 'vue-router'
import { ref } from 'vue'

import { AutomationSimulationDialog } from '@/features/automation-simulate'
import { Button } from '@/shared/ui/button'
import { formatDateTime } from '@/shared/lib/format'
import { Card } from '@/shared/ui/card'
import { ListRow } from '@/shared/ui/list-row'
import { SectionState } from '@/shared/ui/section-state'
import { Switch } from '@/shared/ui/switch'
import { ToggleGroup, ToggleGroupItem } from '@/shared/ui/toggle-group'

import { MINE_FILTERS, useMine } from '../model/use-mine'

/** What fires on its own — repeats, schemes, reminders, alerts — with a switch on each row. */
const { t } = useI18n()
const { list, items, filter, setActive } = useMine()
const route = useRoute()
const simulateOpen = ref(false)

/** A repeat or a scheme opens its card in place; a reminder and an alert lead to their lists. */
function target(item: MineItem) {
  if (item.kind === 'reminder') return { name: 'automations-reminders' }
  if (item.kind === 'alert') return { name: 'automations-alerts' }
  return { query: { ...route.query, card: item.id } }
}

const KIND_ICON = { repeat: Repeat, scheme: Workflow, reminder: CalendarClock, alert: Bell } as const

function subtitle(item: MineItem): string {
  const kind = t(`automations.mine.kind.${item.kind}`)
  if (item.next_at) return `${kind} · ${formatDateTime(item.next_at)}`
  return `${kind} · ${item.kind === 'alert' ? t('automations.mine.byPrice') : item.asks ? t('automations.mine.asks') : t('automations.mine.scheme')}`
}
</script>

<template>
  <div class="grid gap-3" data-tour="automations-list">
    <div class="flex flex-wrap items-center justify-between gap-2">
    <ToggleGroup
      :model-value="filter"
      type="single"
      variant="outline"
      size="sm"
      class="flex-wrap justify-start"
      :aria-label="t('automations.mine.filter')"
      @update:model-value="(value) => value && (filter = value as typeof filter)"
    >
      <ToggleGroupItem v-for="kind in MINE_FILTERS" :key="kind" :value="kind">
        {{ t(`automations.mine.type.${kind}`) }}
      </ToggleGroupItem>
    </ToggleGroup>
      <Button variant="outline" size="sm" data-tour="automation-simulate" @click="simulateOpen = true">
        <FlaskConical class="size-4" />
        {{ t('simulation.open') }}
      </Button>
    </div>
    <SectionState
      :loading="list.isPending.value"
      :error="list.error.value"
      :empty="!items.length"
      :empty-title="t('automations.mine.empty.title')"
      :empty-description="t('automations.mine.empty.phone')"
      :rows="4"
      @retry="list.refetch()"
    >
      <Card size="sm">
        <ul class="divide-y divide-border">
          <li v-for="item in items" :key="`${item.kind}-${item.id}`" :data-mine-row="item.kind">
            <div class="flex items-center gap-2">
              <RouterLink :to="target(item)" class="min-w-0 flex-1 rounded-md focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring">
                <ListRow :title="item.name" :subtitle="subtitle(item)" :icon="KIND_ICON[item.kind]" />
              </RouterLink>
              <Switch
                class="shrink-0"
                :model-value="item.active"
                :aria-label="t('automations.mine.toggle', { name: item.name })"
                @update:model-value="(value) => setActive(item, value)"
              />
            </div>
          </li>
        </ul>
      </Card>
    </SectionState>
    <AutomationSimulationDialog v-model:open="simulateOpen" :automation-id="null" name="" />
  </div>
</template>
