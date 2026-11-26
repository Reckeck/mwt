<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { useRouter } from 'vue-router'
import { useMutation, useQueryClient } from '@tanstack/vue-query'

import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { PLANNING_KEYS } from '@/shared/lib/planning'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

/**
 * Over the editor of an automation's workflow: it is built from the
 * automation form, so it is changed there, or made an ordinary workflow to
 * edit here.
 */
const props = defineProps<{ workflowId: string; name: string }>()
const { t } = useI18n()
const router = useRouter()
const queryClient = useQueryClient()

const detach = useMutation({
  mutationFn: () => call<Workflow>('detach_automation', { id: props.workflowId }),
  onSuccess: () => {
    for (const queryKey of [...PLANNING_KEYS, WORKFLOW_KEYS.list, WORKFLOW_KEYS.one(props.workflowId)])
      void queryClient.invalidateQueries({ queryKey })
  },
  meta: {
    error: 'automation.detach.failed',
    success: () => ({ text: t('automation.detach.done'), description: props.name }),
  },
})
</script>

<template>
  <div
    class="flex flex-wrap items-center gap-2 rounded-lg border border-primary/40 bg-primary/5 px-3 py-2 text-sm"
    data-tour="workflow-automation"
  >
    <span class="min-w-0 flex-1">{{ t('automation.detach.banner') }}</span>
    <Button size="sm" variant="outline" @click="router.push({ name: 'automations', query: { edit: props.workflowId } })">
      {{ t('automation.detach.form') }}
    </Button>
    <Button size="sm" variant="ghost" :disabled="detach.isPending.value" @click="detach.mutate()">
      {{ t('automation.detach.action') }}
    </Button>
  </div>
</template>
