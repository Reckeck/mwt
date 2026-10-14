<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { LEDGER_KEYS, PLANNING_KEYS } from '@/shared/lib/planning'

import type { AutomationRunActionsProps } from './automation-edit.types'

const props = defineProps<AutomationRunActionsProps>()

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
  <div class="flex shrink-0 items-center gap-1">
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
