<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'

import { Badge } from '@/shared/ui/badge'
import { Switch } from '@/shared/ui/switch'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import { mineDoes, mineName, mineRoute, mineWhen } from '../../lib/mine-view'

/**
 * «Мои» on a computer (Auto-1 · 1): Название · Тип · Следующий раз · Что
 * сделает · Вкл. A click on the name opens it; the switch turns it off.
 */
const props = defineProps<{ items: MineItem[]; storeName: (id: string) => string }>()
const emit = defineEmits<{ toggle: [item: MineItem, active: boolean] }>()
const { t } = useI18n()
const route = useRoute()

const TH = 'h-auto px-3 py-2 text-xs font-medium text-muted-foreground'
const rows = computed(() =>
  props.items.map((item) => ({
    item,
    key: `${item.kind}:${item.id}`,
    name: mineName(item),
    when: mineWhen(item, t),
    does: mineDoes(item, props.storeName, t),
    to: mineRoute(item, route.query),
  })),
)
/** The name's link covers the whole row; its focus ring goes round the row. */
const RING = 'focus-visible:after:ring-2 focus-visible:after:ring-ring focus-visible:after:ring-inset'
</script>

<template>
  <div class="overflow-hidden rounded-xl border border-border bg-card shadow-sm" data-mine-table>
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead :class="TH">{{ t('automations.mine.col.name') }}</TableHead>
          <TableHead :class="TH">{{ t('automations.mine.col.type') }}</TableHead>
          <TableHead :class="TH">{{ t('automations.mine.col.next') }}</TableHead>
          <TableHead :class="TH">{{ t('automations.mine.col.does') }}</TableHead>
          <TableHead :class="[TH, 'text-right']">{{ t('automations.mine.col.on') }}</TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="row in rows" :key="row.key" class="relative" :data-mine-row="row.key">
          <TableCell class="max-w-0 w-[37%] px-3 py-2.5">
            <RouterLink
              :to="row.to"
              class="block truncate font-medium after:absolute after:inset-0 focus-visible:outline-none"
              :class="RING"
            >
              {{ row.name }}
            </RouterLink>
          </TableCell>
          <TableCell class="px-3 py-2.5">
            <Badge variant="secondary" class="h-5.5 px-2 font-medium">{{ t(`automations.mine.kind.${row.item.kind}`) }}</Badge>
          </TableCell>
          <TableCell class="px-3 py-2.5 whitespace-nowrap text-muted-foreground">{{ row.when }}</TableCell>
          <TableCell class="px-3 py-2.5 text-sm whitespace-nowrap">{{ row.does }}</TableCell>
          <TableCell class="px-3 py-2.5 text-right">
            <Switch
              class="relative z-10 ml-auto"
              :model-value="row.item.active"
              :aria-label="t('automations.mine.toggle', { name: row.name })"
              @update:model-value="(value: boolean) => emit('toggle', row.item, value)"
            />
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
</template>
