<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { diffTone, qtyText } from '@/entities/shared-event'
import { formatQuantity } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import type { UiAction } from '@/shared/ui/context-menu'
import { Card } from '@/shared/ui/card'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'

import type { ItemRow } from '../../shared-event-items.types'

/**
 * The plan against what was bought: planned items wait with «Купили», bought ones show the
 * difference («8 из 6 шт · +600 ₽»), a purchase outside the plan is marked «вне плана».
 * On a phone the figures move under the name; the buttons keep their place (they never swap).
 */
const props = defineProps<{ rows: ItemRow[]; currency: string; decimals: number; actionsOf: (row: ItemRow) => UiAction[] }>()
const emit = defineEmits<{ buy: [row: ItemRow] }>()
const { t } = useI18n()

const STATE_BADGE = {
  planned: { variant: 'secondary', class: '' },
  bought: { variant: 'outline', class: 'border-primary text-primary' },
  unplanned: { variant: 'warning', class: '' },
  cancelled: { variant: 'outline', class: 'text-muted-foreground' },
} as const

/** The small line under the name on a phone: the plan before the purchase, the plan against the fact after it. */
function phoneLine(row: ItemRow): string {
  if (row.fact.state === 'planned') return row.item.plan ? `${formatQuantity(row.item.plan.qty)} ${row.item.unit}` : ''
  return row.summary
}

const TONE_CLASS = { over: 'text-destructive font-semibold', under: 'text-success font-semibold', even: '' } as const
</script>

<template>
  <Card class="p-0" data-tour="shared-items-table">
    <Table class="tabular-nums">
      <TableHeader>
        <TableRow>
          <TableHead class="text-xs text-muted-foreground">{{ t('sharedEvents.items.head.item') }}</TableHead>
          <TableHead class="w-28 text-xs text-muted-foreground max-md:hidden">{{ t('sharedEvents.items.head.qty') }}</TableHead>
          <TableHead class="w-28 text-right text-xs text-muted-foreground max-md:hidden">{{ t('sharedEvents.items.head.plan') }}</TableHead>
          <TableHead class="w-28 text-right text-xs text-muted-foreground max-md:hidden">{{ t('sharedEvents.items.head.fact') }}</TableHead>
          <TableHead class="text-xs text-muted-foreground max-md:hidden">{{ t('sharedEvents.items.head.status') }}</TableHead>
          <TableHead class="w-px"><span class="sr-only">{{ t('actions.more') }}</span></TableHead>
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow v-for="row in props.rows" :key="row.item.id" :class="cn('align-top md:align-middle', row.fact.state === 'cancelled' && 'text-muted-foreground')">
          <TableCell class="min-w-0 whitespace-normal px-3 py-2.5">
            <div class="grid min-w-0 gap-0.5">
              <span :class="cn('truncate font-medium', row.fact.state === 'cancelled' && 'line-through')">{{ row.item.title }}</span>
              <span v-if="row.caption" class="truncate text-xs text-primary">{{ row.caption }}</span>
              <span class="flex flex-wrap items-center gap-x-2 gap-y-1 pt-0.5 text-xs text-muted-foreground md:hidden">
                <Badge :variant="STATE_BADGE[row.fact.state].variant" :class="STATE_BADGE[row.fact.state].class">{{ t(`sharedEvents.items.state.${row.fact.state}`) }}</Badge>
                <span>{{ phoneLine(row) }}</span>
              </span>
            </div>
          </TableCell>
          <TableCell class="text-sm max-md:hidden">
            {{ row.fact.planQty !== null ? formatQuantity(row.fact.planQty) : formatQuantity(row.fact.bought) }} {{ row.item.unit }}
          </TableCell>
          <TableCell class="text-right text-sm max-md:hidden">
            <Money v-if="row.fact.planTotal !== null" :amount="row.fact.planTotal" :currency="props.currency" :decimals="props.decimals" :convert="false" />
            <span v-else class="text-muted-foreground">—</span>
          </TableCell>
          <TableCell class="text-right text-sm max-md:hidden">
            <Money v-if="row.view.bought" :amount="row.fact.cost" :currency="props.currency" :decimals="props.decimals" :convert="false" />
            <span v-else class="text-muted-foreground">—</span>
          </TableCell>
          <TableCell class="whitespace-normal text-xs text-muted-foreground max-md:hidden">
            <span class="flex flex-wrap items-center gap-x-2 gap-y-1" :aria-label="row.summary || undefined">
              <Badge :variant="STATE_BADGE[row.fact.state].variant" :class="STATE_BADGE[row.fact.state].class">{{ t(`sharedEvents.items.state.${row.fact.state}`) }}</Badge>
              <template v-if="row.fact.state === 'bought' || row.fact.state === 'unplanned'">
                <span>{{ qtyText(row.fact, row.item.unit, formatQuantity) }}</span>
                <template v-if="row.fact.diffMoney !== null">
                  <span aria-hidden="true">·</span>
                  <Money
                    :amount="row.fact.diffMoney"
                    :currency="props.currency"
                    :decimals="props.decimals"
                    :convert="false"
                    signed
                    :class="TONE_CLASS[diffTone(row.fact.diffMoney)]"
                  />
                </template>
                <template v-else>
                  <span aria-hidden="true">·</span>
                  <Money :amount="row.fact.cost" :currency="props.currency" :decimals="props.decimals" :convert="false" />
                </template>
              </template>
            </span>
          </TableCell>
          <TableCell class="w-px px-2 py-2">
            <div class="flex items-center justify-end gap-2">
              <span v-if="row.boughtBy" class="hidden text-xs text-muted-foreground lg:inline">{{ row.boughtBy }}</span>
              <Button
                v-if="row.fact.state === 'planned'"
                variant="outline"
                size="sm"
                class="whitespace-nowrap"
                data-tour="shared-buy"
                @click="emit('buy', row)"
              >
                {{ t('sharedEvents.items.buy') }}
              </Button>
              <ActionMenu :actions="props.actionsOf(row)" :label="t('actions.more')" :sheet="{ title: row.item.title }" />
            </div>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </Card>
</template>
