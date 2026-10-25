<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { AmountInput } from '@/shared/ui/amount-input'
import { Button } from '@/shared/ui/button'
import { assetKey } from '@/shared/lib/asset-ref'
import { signClass } from '@/shared/lib/format'

import { badRow, difference } from '../../lib/adjustment-rows'
import type { AdjustmentRow } from '../../store-adjust.types'

/** Asset, ledger balance, what is really there, and the difference — one row per asset. */
const rows = defineModel<AdjustmentRow[]>('rows', { required: true })
const props = defineProps<{ invalid: boolean; loaded: boolean }>()

const { t } = useI18n()
/**
 * Five columns from 36rem; narrower (a phone), two lines per asset: the asset
 * and its delete button, then ledger · actual · difference. The fixed 26rem of
 * tracks cut the difference and the button off on a phone.
 */
const GRID =
  'grid grid-cols-[minmax(0,1fr)_minmax(0,1.3fr)_minmax(0,1fr)] items-center gap-x-2 gap-y-1 @xl:grid-cols-[minmax(0,1fr)_8rem_9rem_7rem_2rem]'
/** The asset spans the first line on a phone; the last cell sits at its end. */
const ASSET = '@max-xl:col-span-2'
const LAST = '@max-xl:col-start-3 @max-xl:row-start-1 justify-self-end'

function removeRow(index: number) {
  rows.value.splice(index, 1)
}
</script>

<template>
  <div
    class="@container overflow-hidden rounded-lg border"
    :class="props.invalid ? 'border-destructive' : 'border-border'"
    :data-invalid="props.invalid ? '' : undefined"
    data-tour="adjustment-rows"
  >
    <div :class="[GRID, 'border-b border-border bg-muted/40 px-3 py-2 text-xs text-muted-foreground']">
      <span :class="ASSET">{{ t('adjustment.asset') }}</span>
      <span class="text-right">{{ t('adjustment.ledger') }}</span>
      <span>{{ t('adjustment.actual') }}</span>
      <span class="text-right">{{ t('adjustment.difference') }}</span>
      <span :class="LAST" />
    </div>
    <p v-if="!rows.length && props.loaded" class="px-3 py-4 text-sm text-muted-foreground">{{ t('adjustment.empty') }}</p>
    <div v-for="(row, index) in rows" :key="assetKey(row.asset)" :class="[GRID, 'border-b border-border px-3 py-1.5 last:border-b-0']">
      <span class="truncate text-sm font-medium" :class="ASSET" :title="row.asset.name">{{ row.asset.symbol }}</span>
      <span class="truncate text-right text-sm tabular-nums text-muted-foreground" :title="row.before">
        <Money :amount="row.before" :currency="row.asset.symbol" />
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
        <Money v-if="difference(row) !== null" :amount="difference(row)" :currency="row.asset.symbol" signed />
        <template v-else>—</template>
      </span>
      <Button
        v-if="row.before === '0'"
        type="button"
        variant="ghost"
        size="icon-sm"
        :class="LAST"
        :aria-label="t('actions.delete')"
        @click="removeRow(index)"
      >
        <Trash2 class="size-3.5" />
      </Button>
      <span v-else :class="LAST" />
    </div>
  </div>
</template>
