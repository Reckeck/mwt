<script setup lang="ts">
import { computed, nextTick, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Download, ExternalLink, Pencil, Trash2 } from 'lucide-vue-next'

import { AttachmentIcon, AttachmentRow, attachmentLook } from '@/entities/attachment'
import { ActionMenu } from '@/shared/ui/action-menu'
import type { UiAction } from '@/shared/ui/context-menu'
import { Input } from '@/shared/ui/input'

/**
 * A row with its «⋯» (Attach 1): Открыть, Сохранить как…, Переименовать,
 * Удалить. Renaming turns the name into a field: Enter keeps it, Esc drops it.
 */
const props = defineProps<{ item: Attachment }>()
const emit = defineEmits<{ open: []; save: []; rename: [title: string]; remove: [] }>()

const { t } = useI18n()
const renaming = ref(false)
const draft = ref('')
const field = ref<InstanceType<typeof Input> | null>(null)

function startRename() {
  draft.value = props.item.title
  renaming.value = true
  void nextTick(() => {
    const input = field.value?.$el as HTMLInputElement | undefined
    input?.focus()
    input?.select()
  })
}

function finishRename() {
  if (!renaming.value) return
  renaming.value = false
  const title = draft.value.trim()
  if (title && title !== props.item.title) emit('rename', title)
}

const actions = computed<UiAction[]>(() => [
  { id: 'open', label: t('attachments.open'), icon: ExternalLink, run: () => emit('open') },
  { id: 'save', label: t('attachments.saveAs'), icon: Download, hidden: props.item.kind === 'link', run: () => emit('save') },
  { id: 'rename', label: t('attachments.rename'), icon: Pencil, run: startRename },
  { id: 'remove', label: t('actions.delete'), icon: Trash2, destructive: true, separated: true, run: () => emit('remove') },
])
</script>

<template>
  <div v-if="renaming" class="flex min-h-11 items-center gap-3">
    <AttachmentIcon :look="attachmentLook(props.item)" />
    <Input
      ref="field"
      v-model="draft"
      class="h-9 flex-1"
      :aria-label="t('attachments.rename')"
      @keydown.enter.prevent="finishRename"
      @keydown.esc.stop.prevent="renaming = false"
      @blur="finishRename"
    />
  </div>
  <AttachmentRow v-else :item="props.item" dated @open="emit('open')">
    <ActionMenu :actions="actions" :label="t('actions.more')" :sheet="{ title: props.item.title }" />
  </AttachmentRow>
</template>
