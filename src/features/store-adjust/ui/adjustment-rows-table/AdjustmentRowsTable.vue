<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { assetKey } from '@/shared/lib/asset-ref'
import { signClass } from '@/shared/lib/format'

import { badRow, difference, shownBalance } from '../../lib/adjustment-rows'
import type { AdjustmentRow } from '../../store-adjust.types'

/** Asset, ledger balance, what is really there, and the difference — one row per asset. */
const rows = defineModel<AdjustmentRow[]>('rows', { required: true })
const props = defineProps<{ invalid: boolean; loaded: boolean }>()

const { t } = useI18n()
const GRID = 'grid grid-cols-[minmax(0,1fr)_8rem_9rem_7rem_2rem] items-center gap-2'

function removeRow(index: number) {
  rows.value.splice(index, 1)
}
</script>

<template>
  <div
    class="overflow-hidden rounded-lg border"
    :class="props.invalid ? 'border-destructive' : 'border-border'"
    :data-invalid="props.invalid ? '' : undefined"
    data-tour="adjustment-rows"
  >
    <div :class="[GRID, 'border-b border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground']">
      <span>{{ t('adjustment.asset') }}</span>
      <span class="text-right">{{ t('adjustment.ledger') }}</span>
      <span>{{ t('adjustment.actual') }}</span>
      <span class="text-right">{{ t('adjustment.difference') }}</span>
      <span />
    </div>
    <p v-if="!rows.length && props.loaded" class="px-3 py-4 text-sm text-muted-foreground">{{ t('adjustment.empty') }}</p>
    <div v-for="(row, index) in rows" :key="assetKey(row.asset)" :class="[GRID, 'border-b border-border px-3 py-1.5 last:border-b-0']">
      <span class="truncate text-sm font-medium" :title="row.asset.name">{{ row.asset.symbol }}</span>
      <span class="truncate text-right text-sm tabular-nums text-muted-foreground" :title="row.before">
        {{ shownBalance(row.before, row.asset) }}
      </span>
      <AmountInput
        v-model="row.actual"
        :placeholder="row.before"
        :class="badRow(row) && 'border-destructive'"
        :aria-invalid="badRow(row)"
        :aria-label="t('adjustment.actualFor', { asset: row.asset.symbol })"
      />
      <span
        class="truncate text-right text-sm tabular-nums"
        :class="difference(row) === null ? 'text-muted-foreground' : signClass(difference(row))"
      >
        {{ shownBalance(difference(row), row.asset, true) }}
      </span>
      <Button
        v-if="row.before === '0'"
        type="button"
        variant="ghost"
        size="icon"
        class="size-7"
        :aria-label="t('actions.delete')"
        @click="removeRow(index)"
      >
        <Trash2 class="size-3.5" />
      </Button>
      <span v-else />
    </div>
  </div>
</template>
