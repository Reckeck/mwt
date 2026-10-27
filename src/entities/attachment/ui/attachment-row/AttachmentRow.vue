<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ArrowUpRight } from 'lucide-vue-next'

import { formatDayMonthShort } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'

import { attachmentLook, formatBytes, linkShown } from '../../lib/attachment-format'
import type { AttachmentRowProps } from './attachment-row.types'
import AttachmentIcon from './AttachmentIcon.vue'

/**
 * One file or link (Attach 1): its kind, its name, «PDF · 84 КБ · добавлен
 * 21 окт» or «Ссылка · t.me/…». The whole row opens it; actions go in the slot.
 */
const props = defineProps<AttachmentRowProps>()
const emit = defineEmits<{ open: [] }>()
defineSlots<{ default?(): unknown }>()

const { t } = useI18n()

const look = computed(() => attachmentLook(props.item))
const line = computed(() => {
  const item = props.item
  if (item.kind === 'link') return [t('attachments.look.link'), linkShown(item.url ?? '')].join(' · ')
  const size = formatBytes(item.size, { b: t('attachments.bytes'), kb: t('attachments.kb'), mb: t('attachments.mb') })
  return [
    t(`attachments.look.${look.value}`),
    size,
    props.dated ? t('attachments.added', { date: formatDayMonthShort(item.created_at) }) : '',
  ]
    .filter(Boolean)
    .join(' · ')
})
</script>

<template>
  <div class="flex min-h-11 min-w-0 items-center gap-3" data-attachment-row>
    <Button
      variant="ghost"
      size="block"
      class="min-w-0 flex-1 gap-3 px-0 hover:bg-transparent"
      :title="props.item.kind === 'link' ? props.item.url ?? undefined : props.item.title"
      @click="emit('open')"
    >
      <AttachmentIcon :look="look" />
      <span class="grid min-w-0 flex-1">
        <span class="flex min-w-0 items-center gap-1 text-sm">
          <span class="truncate font-medium">{{ props.item.title }}</span>
          <ArrowUpRight v-if="props.item.kind === 'link'" class="size-3.5 shrink-0 text-muted-foreground" />
        </span>
        <span class="truncate text-xs text-muted-foreground">{{ line }}</span>
      </span>
    </Button>
    <slot />
  </div>
</template>
