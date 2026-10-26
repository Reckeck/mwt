<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Label } from '@/shared/ui/label'
import { Select, SelectItem } from '@/shared/ui/select'
import { formatDateTime } from '@/shared/lib/format'

import { useTxForm } from '../../model/use-tx-form'

/** Recent Bybit fills of the wallet: a click fills the trade in. */
const { t } = useI18n()
const { form, lookup, bybitCategory, applyFill } = useTxForm()

/** Bybit stamps a fill in epoch milliseconds, as text. */
const fillTime = (raw: string) => formatDateTime(/^\d{10,}$/.test(raw) ? new Date(Number(raw)).toISOString() : raw)
</script>

<template>
  <div class="grid gap-1.5">
    <div class="flex items-center justify-between gap-2">
      <Label>{{ t('tx.bybit.recent') }}</Label>
      <div class="flex items-center gap-2">
        <Select v-if="form.kind === 'trade_spot'" v-model="bybitCategory" class="h-8 w-28 text-xs">
          <SelectItem value="spot">{{ t('tx.bybit.category.spot') }}</SelectItem>
        </Select>
        <Button
          type="button"
          variant="ghost"
          size="sm"
          :disabled="lookup.executionsLoading"
          @click="lookup.loadExecutions(form.kind === 'trade_futures' ? 'linear' : 'spot')"
        >
          {{ t('actions.refresh') }}
        </Button>
      </div>
    </div>
    <p v-if="lookup.keyNotice" class="text-xs text-muted-foreground">{{ t(`tx.bybit.${lookup.keyNotice}`) }}</p>
    <div v-else-if="lookup.executions.length" class="grid max-h-40 gap-1 overflow-y-auto">
      <Button
        v-for="exec in lookup.executions"
        :key="exec.exec_time + exec.exec_qty + exec.exec_price"
        type="button"
        variant="outline"
        size="sm"
        class="h-auto min-w-0 flex-wrap justify-between gap-x-2 gap-y-0.5 px-2 py-1.5 text-left text-xs font-normal whitespace-normal"
        @click="applyFill(exec)"
      >
        <span class="font-medium">{{ exec.symbol }}</span>
        <span :class="exec.side === 'Buy' ? 'text-success' : 'text-destructive'">{{ exec.side }}</span>
        <span class="text-muted-foreground">{{ exec.exec_qty }} @ {{ exec.exec_price }}</span>
        <span class="text-muted-foreground">{{ fillTime(exec.exec_time) }}</span>
      </Button>
    </div>
  </div>
</template>
