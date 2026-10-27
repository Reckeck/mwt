<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { FileWarning, X } from 'lucide-vue-next'

import { AttachmentIcon } from '@/entities/attachment'
import { Button } from '@/shared/ui/button'

import type { FailedFile, PendingFile } from '../../attachment-edit.types'

/**
 * Files on their way in (Attach 3) and the ones that did not make it
 * (Attach 7): a bar with «Отменить», or the reason in red with «Убрать».
 */
const props = defineProps<{ pending: PendingFile[]; failed: FailedFile[] }>()
const emit = defineEmits<{ cancel: [key: number]; dismiss: [key: number] }>()

const { t } = useI18n()
</script>

<template>
  <div v-for="row in props.pending" :key="`p${row.key}`" class="flex min-h-13 items-center gap-3" data-attachment-pending>
    <AttachmentIcon look="image" />
    <span class="grid min-w-0 flex-1 gap-1">
      <span class="flex min-w-0 items-center gap-1.5 text-sm">
        <span class="flex-1 truncate font-medium">{{ row.name }}</span>
        <span class="shrink-0 text-xs text-muted-foreground">{{ t(row.cancelled ? 'attachments.cancelling' : 'attachments.sealing') }}</span>
      </span>
      <span class="h-1 overflow-hidden rounded-full bg-secondary">
        <span class="block h-full w-1/3 animate-pulse rounded-full bg-primary" />
      </span>
    </span>
    <Button
      variant="ghost"
      size="icon"
      class="text-muted-foreground"
      :disabled="row.cancelled"
      :aria-label="t('actions.cancel')"
      :title="t('actions.cancel')"
      @click="emit('cancel', row.key)"
    >
      <X class="size-4" />
    </Button>
  </div>
  <div v-for="row in props.failed" :key="`f${row.key}`" class="flex min-h-11 items-center gap-3" role="alert" data-attachment-failed>
    <span class="flex size-8 shrink-0 items-center justify-center rounded-lg bg-destructive/12 text-destructive">
      <FileWarning class="size-4" />
    </span>
    <span class="grid min-w-0 flex-1">
      <span class="truncate text-sm font-medium">{{ row.name }}</span>
      <span class="truncate text-xs text-destructive" :title="row.reason">{{ row.reason }}</span>
    </span>
    <Button variant="link" size="sm" class="px-1" @click="emit('dismiss', row.key)">{{ t('attachments.dismiss') }}</Button>
  </div>
</template>
