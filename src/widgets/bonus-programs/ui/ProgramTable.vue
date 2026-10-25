<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Minus, Settings2 } from 'lucide-vue-next'

import { Money, MoneyPhrase } from '@/entities/currency'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import type { UiAction } from '@/shared/ui/context-menu'
import { Table, TableBody, TableCell, TableHead, TableHeader, TableRow } from '@/shared/ui/table'
import { cn } from '@/shared/lib/utils'

import type { PointsMode, ProgramRow } from '../bonus-programs.types'

/** The programmes as a table (Ops-2 · 12): «Начислить» on the row, the rest in «⋯». */
const props = defineProps<{ rows: ProgramRow[]; baseCurrency: string; highlighted: string }>()
const emit = defineEmits<{ points: [store: Store, mode: PointsMode]; settings: [store: Store] }>()
const { t, te } = useI18n()

/** «В долларах», «В рублях»; other bases by their code. */
const inBase = (code: string) =>
  te(`bonuses.inBase.${code}`) ? t(`bonuses.inBase.${code}`) : t('bonuses.inBase.other', { code: code.toUpperCase() })

const actions = (store: Store): UiAction[] => [
  { id: 'spend', label: t('loyalty.spend'), icon: Minus, run: () => emit('points', store, 'spend') },
  { id: 'settings', label: t('bonuses.settings'), icon: Settings2, run: () => emit('settings', store) },
]
</script>

<template>
  <div class="overflow-hidden rounded-xl border border-border bg-card shadow-sm">
    <Table>
      <TableHeader>
        <TableRow>
          <TableHead>{{ t('bonuses.program') }}</TableHead>
          <TableHead class="text-right">{{ t('bonuses.balance') }}</TableHead>
          <TableHead class="text-right">{{ inBase(props.baseCurrency) }}</TableHead>
          <TableHead class="w-0" />
        </TableRow>
      </TableHeader>
      <TableBody>
        <TableRow
          v-for="row in props.rows"
          :key="row.store.id"
          :class="cn(row.store.id === props.highlighted && 'bg-primary/5 ring-2 ring-inset ring-primary')"
          :data-program="row.store.id"
        >
          <TableCell class="max-w-0 py-3">
            <p class="truncate font-medium">{{ row.store.name }}</p>
            <p class="truncate text-xs text-muted-foreground">
              <MoneyPhrase :parts="row.subtitle" />
            </p>
            <p v-if="row.expiring" :class="cn('truncate text-xs', row.soon ? 'text-warning' : 'text-muted-foreground')">
              {{ row.expiring }}
            </p>
          </TableCell>
          <TableCell class="text-right font-semibold whitespace-nowrap tabular-nums">{{ row.points }}</TableCell>
          <TableCell class="text-right whitespace-nowrap text-muted-foreground">
            ≈ <Money :amount="row.valueBase" :currency="props.baseCurrency" :decimals="0" />
          </TableCell>
          <TableCell class="w-0">
            <div class="flex items-center justify-end gap-1">
              <Button variant="outline" size="sm" :disabled="!row.store.loyalty" @click="emit('points', row.store, 'credit')">
                {{ t('loyalty.credit') }}
              </Button>
              <ActionMenu :actions="actions(row.store)" :label="t('bonuses.actions')" />
            </div>
          </TableCell>
        </TableRow>
      </TableBody>
    </Table>
  </div>
</template>
