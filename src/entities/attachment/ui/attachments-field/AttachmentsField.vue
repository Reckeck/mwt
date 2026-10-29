<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { X } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { notify } from '@/shared/lib/notify'

import { MAX_ATTACHMENT_BYTES } from '../../attachment.constants'
import { attachmentLook, formatBytes } from '../../lib/attachment-format'
import { AttachmentIcon } from '../attachment-row'
import type { LinkDraft, StagedAttachment } from '../../attachment.types'
import AttachmentAdd from '../attachment-add/AttachmentAdd.vue'

/**
 * `Files and links` in a form's `More` (Attach 5): what is picked waits here as
 * chips and is attached once the form saves; `Cancel` drops it with the form.
 */
const staged = defineModel<StagedAttachment[]>({ required: true })

const { t } = useI18n()
const linkOpen = ref(false)
let next = 0

const size = (bytes: number) =>
  formatBytes(bytes, { b: t('attachments.bytes'), kb: t('attachments.kb'), mb: t('attachments.mb') })

function addFiles(files: File[]) {
  for (const file of files) {
    if (file.size > MAX_ATTACHMENT_BYTES) {
      const why = t('attachments.tooLarge', { size: size(file.size), max: size(MAX_ATTACHMENT_BYTES) })
      notify.error(null, t('attachments.notAttached', { why }), { description: file.name })
      continue
    }
    staged.value = [...staged.value, { key: next++, kind: 'file', name: file.name || t('attachments.pastedName'), file }]
  }
}

function addLink({ url, title }: LinkDraft) {
  staged.value = [...staged.value, { key: next++, kind: 'link', url, title }]
  linkOpen.value = false
}

const drop = (key: number) => (staged.value = staged.value.filter((item) => item.key !== key))
const lookOf = (item: StagedAttachment) =>
  item.kind === 'link' ? 'link' : attachmentLook({ kind: 'file', mime: item.file.type || null, title: item.name })
</script>

<template>
  <FormField :label="t('attachments.title')" class="col-span-full" data-attachments-field>
    <div class="flex flex-wrap items-center gap-1.5">
      <span
        v-for="item in staged"
        :key="item.key"
        class="inline-flex h-7 max-w-full items-center gap-1.5 rounded-full border pr-1 pl-2.5 text-sm pointer-coarse:h-10"
      >
        <AttachmentIcon :look="lookOf(item)" bare />
        <span class="min-w-0 truncate">{{ item.kind === 'link' ? item.title || item.url : item.name }}</span>
        <Button
          variant="ghost"
          size="icon-xs"
          class="shrink-0 rounded-full text-muted-foreground hover:bg-transparent hover:text-foreground"
          :aria-label="t('attachments.dismiss')"
          :title="t('attachments.dismiss')"
          @click="drop(item.key)"
        >
          <X class="size-3.5" />
        </Button>
      </span>
      <AttachmentAdd v-model:link-open="linkOpen" :label="t('attachments.fileOrLink')" @files="addFiles" @link="addLink" />
    </div>
  </FormField>
</template>
