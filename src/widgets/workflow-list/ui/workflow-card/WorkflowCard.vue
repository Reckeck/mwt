<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Copy, Pencil, Play, Trash2 } from 'lucide-vue-next'

import { OutcomeBadge } from '@/entities/workflow'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { ConfirmButton } from '@/shared/ui/confirm-button'
import { ConfirmDialog } from '@/shared/ui/confirm-dialog'
import { EntityLink } from '@/shared/ui/entity-link'
import { Switch } from '@/shared/ui/switch'
import { formatDateTime } from '@/shared/lib/format'
import { useIsPhone } from '@/shared/lib/phone'

/** One workflow: on or off, its last run; open, run, share, delete (four buttons, or one «⋯» on a phone). */
const props = defineProps<{ workflow: Workflow; lastRun: WorkflowRun | null; switching: boolean; running: boolean }>()
const emit = defineEmits<{ active: [active: boolean]; edit: []; run: []; export: []; remove: [] }>()
const { t } = useI18n()
const phone = useIsPhone()

const confirmOpen = ref(false)
function confirmRemove() {
  confirmOpen.value = false
  emit('remove')
}
const menu = computed(() => [
  { id: 'edit', label: t('workflow.list.edit'), icon: Pencil, run: () => emit('edit') },
  { id: 'run', label: t('workflow.runNow'), icon: Play, disabled: props.running, run: () => emit('run') },
  { id: 'export', label: t('workflow.list.export'), icon: Copy, run: () => emit('export') },
  { id: 'delete', label: t('workflow.list.delete'), icon: Trash2, destructive: true, separated: true, run: () => (confirmOpen.value = true) },
])
</script>

<template>
  <Card data-tour="workflow-card">
    <div class="flex items-start gap-3">
      <!-- On a touch screen the switch steps off the card's edge, so its hit area fits inside. -->
      <Switch
        class="pointer-coarse:my-2.5"
        :model-value="props.workflow.active"
        :aria-label="t('workflow.active')"
        :disabled="props.switching"
        @update:model-value="(active) => emit('active', Boolean(active))"
      />
      <div class="grid min-w-0 flex-1 gap-1">
        <span class="flex flex-wrap items-center gap-2">
          <EntityLink :target="{ kind: 'workflow', id: props.workflow.id }" :label="props.workflow.name" plain class="font-medium" />
          <!-- Built from the automation form: changed there. -->
          <Badge v-if="props.workflow.automation" variant="secondary" :title="t('workflow.list.automationHint')">
            {{ t('workflow.list.automation') }}
          </Badge>
        </span>
        <p v-if="props.workflow.description" class="text-sm text-muted-foreground">{{ props.workflow.description }}</p>
        <p class="flex flex-wrap items-center gap-2 text-xs text-muted-foreground">
          <template v-if="props.lastRun">
            <OutcomeBadge :outcome="props.lastRun.outcome" />
            {{ formatDateTime(props.lastRun.started_at) }}
          </template>
          <template v-else>{{ t('workflow.list.neverRan') }}</template>
        </p>
      </div>
      <ActionMenu v-if="phone" :actions="menu" :label="t('workflow.list.actions')" />
      <div v-else class="flex shrink-0 items-center gap-1">
        <Button variant="ghost" size="icon" :aria-label="t('workflow.list.edit')" @click="emit('edit')">
          <Pencil class="size-4" />
        </Button>
        <Button variant="ghost" size="icon" :aria-label="t('workflow.runNow')" :disabled="props.running" @click="emit('run')">
          <Play class="size-4" />
        </Button>
        <Button variant="ghost" size="icon" :aria-label="t('workflow.list.export')" @click="emit('export')">
          <Copy class="size-4" />
        </Button>
        <ConfirmButton
          :label="t('workflow.list.delete')"
          :confirm-label="t('actions.confirmDeleteNamed', { name: props.workflow.name })"
          @confirm="emit('remove')"
        >
          <Trash2 class="size-4" />
        </ConfirmButton>
      </div>
    </div>
    <ConfirmDialog
      v-if="phone"
      v-model:open="confirmOpen"
      :title="t('workflow.list.delete')"
      :description="t('actions.confirmDeleteNamed', { name: props.workflow.name })"
      @confirm="confirmRemove"
    />
  </Card>
</template>
