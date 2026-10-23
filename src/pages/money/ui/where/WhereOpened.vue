<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, type RouteLocationRaw } from 'vue-router'

import { formatDayMonthShort } from '@/shared/lib/format'
import { TableCell, TableRow } from '@/shared/ui/table'

import { exchangeText, whereItemRoute } from '../../lib/where-view'

/**
 * An opened «Между своими» (Paths 2 · 9): the operations behind the row, each
 * with its day, route and sum, then «Открыть эти 3 в Операциях →». A table's
 * rows on a computer, a stack inside the card on a phone.
 */
const props = defineProps<{ row: WhereRow; to: RouteLocationRaw; phone?: boolean }>()
const { t } = useI18n()

const items = computed(() =>
  [...props.row.items]
    .sort((left, right) => right.at.localeCompare(left.at))
    .map((item, index) => ({
    key: `${item.at}-${index}`,
    day: formatDayMonthShort(item.at),
    route: whereItemRoute(props.row.kind, item),
    sum: exchangeText(item),
  })),
)
const openLabel = computed(() =>
  props.phone ? t('money.where.openShort') : t('money.where.openThese', { count: props.row.count }),
)
</script>

<template>
  <template v-if="!props.phone">
    <TableRow v-for="item in items" :key="item.key" class="bg-muted/40 text-xs hover:bg-muted/40" data-where-item>
      <TableCell class="px-3 text-right text-muted-foreground tabular-nums">{{ item.day }}</TableCell>
      <TableCell class="max-w-0 truncate px-3">{{ item.route }}</TableCell>
      <TableCell class="px-3 text-right whitespace-nowrap tabular-nums">{{ item.sum }}</TableCell>
    </TableRow>
    <TableRow class="bg-muted/40 hover:bg-muted/40">
      <TableCell colspan="3" class="px-3 py-2 text-right">
        <RouterLink :to="props.to" class="text-xs text-primary hover:underline" data-where-open-all>{{ openLabel }} →</RouterLink>
      </TableCell>
    </TableRow>
  </template>
  <div v-else class="grid gap-2 border-t border-dashed pt-2 pb-1">
    <div v-for="item in items" :key="item.key" class="flex items-baseline justify-between gap-2 ps-3 text-xs" data-where-item>
      <span class="grid min-w-0">
        <span class="truncate">{{ item.route }}</span>
        <span class="text-muted-foreground">{{ item.day }}</span>
      </span>
      <span class="shrink-0 tabular-nums">{{ item.sum }}</span>
    </div>
    <RouterLink :to="props.to" class="touch-hit-lg ps-3 text-xs text-primary" data-where-open-all>{{ openLabel }} →</RouterLink>
  </div>
</template>
