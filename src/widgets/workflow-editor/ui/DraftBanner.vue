<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { formatDateTime } from '@/shared/lib/format'

/** Edits kept aside earlier: open them on the canvas or throw them away. */
defineProps<{ savedAt: string; dropping: boolean }>()
const emit = defineEmits<{ open: []; drop: [] }>()
const { t } = useI18n()
</script>

<template>
  <div
    class="flex flex-wrap items-center gap-2 rounded-lg border border-primary/40 bg-primary/5 px-3 py-2 text-sm"
    data-tour="workflow-draft"
  >
    <span class="min-w-0 flex-1">{{ t('workflow.draft.found', { when: formatDateTime(savedAt) }) }}</span>
    <Button size="sm" variant="outline" @click="emit('open')">{{ t('workflow.draft.open') }}</Button>
    <Button size="sm" variant="ghost" :disabled="dropping" @click="emit('drop')">
      {{ t('workflow.draft.drop') }}
    </Button>
  </div>
</template>
