<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

import type { AutomationRunActionsProps } from './automation-edit.types'
import { AutomationAmountDialog } from './ui/automation-amount-dialog'

/** What a run still waits for: `Retry / Skip` a refusal, or the real amount of an approximate one. */
const props = withDefaults(defineProps<AutomationRunActionsProps>(), { name: '' })
const amountOpen = ref(false)
/** The operation booked approximately, when that is what the run waits for. */
const approximate = computed(() =>
  props.run.estimated && props.run.outcome !== 'skipped' && 'created' in props.run.outcome
    ? props.run.outcome.created.tx_id
    : null,
)

const { t } = useI18n()
const queryClient = useQueryClient()

function refresh() {
  for (const queryKey of [...PLANNING_KEYS, ...LEDGER_KEYS]) void queryClient.invalidateQueries({ queryKey })
}

/** Funds are checked again now; the transaction is dated now. */
const retry = useMutation({
  mutationFn: () => call<Transaction>('retry_automation_run', { runId: props.run.id }),
  onSuccess: () => {
    refresh()
  },
  meta: { error: 'automation.run.retryFailed', success: 'automation.run.retried' },
})

const skip = useMutation({
  mutationFn: () => call<AutomationRun>('skip_automation_run', { runId: props.run.id }),
  onSuccess: () => {
    refresh()
  },
  meta: { error: 'automation.run.skipFailed', success: 'automation.run.skipped' },
})
</script>

<template>
  <div v-if="approximate" class="flex shrink-0 items-center gap-1">
    <Button variant="outline" size="sm" data-tour="automation-confirm-amount" @click="amountOpen = true">
      {{ t('automation.amount.action') }}
    </Button>
    <AutomationAmountDialog
      v-model:open="amountOpen"
      :tx-id="approximate"
      :name="props.name"
      :estimate="props.run.amount ?? ''"
      :symbol="props.run.symbol"
    />
  </div>
  <div v-else class="flex shrink-0 items-center gap-1">
    <Button variant="outline" size="sm" :disabled="retry.isPending.value" @click="retry.mutate()">
      {{ t('automation.run.retry') }}
    </Button>
    <Button
      v-if="props.run.outcome !== 'skipped'"
      variant="ghost"
      size="sm"
      :disabled="skip.isPending.value"
      @click="skip.mutate()"
    >
      {{ t('automation.run.skip') }}
    </Button>
  </div>
</template>
