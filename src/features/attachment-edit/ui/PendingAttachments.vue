<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Link2, Paperclip, X } from 'lucide-vue-next'

import { notify } from '@/shared/lib/notify'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'

import { MAX_ATTACHMENT_BYTES, type PendingAttachment, looksLikeLink, pendingLabel, readPendingFile } from '../lib/pending-attachment'

/**
 * Files and links of an operation that is not saved yet: they wait in the list and are attached
 * right after the save (`useAttachPending`). The size and the address are checked on add.
 */
const model = defineModel<PendingAttachment[]>({ default: () => [] })
const { t } = useI18n()
const MEGABYTE = 1024 * 1024
const picker = ref<HTMLInputElement | null>(null)
const link = ref('')
const linkOpen = ref(false)
const linkBad = ref(false)

async function addFiles(event: Event) {
  const input = event.target as HTMLInputElement
  const picked = [...(input.files ?? [])]
  input.value = ''
  for (const file of picked) {
    if (file.size > MAX_ATTACHMENT_BYTES) {
      const size = `${Math.ceil(file.size / MEGABYTE)} ${t('attachments.mb')}`
      const max = `${MAX_ATTACHMENT_BYTES / MEGABYTE} ${t('attachments.mb')}`
      notify.error(null, t('attachments.notAttached', { why: t('attachments.tooLarge', { size, max }) }))
      continue
    }
    model.value = [...model.value, await readPendingFile(file)]
  }
}

function addLink() {
  const url = link.value.trim()
  if (!looksLikeLink(url)) {
    linkBad.value = true
    return
  }
  model.value = [...model.value, { kind: 'link', url }]
  link.value = ''
  linkBad.value = false
  linkOpen.value = false
}

const removeAt = (index: number) => (model.value = model.value.filter((_, at) => at !== index))
</script>

<template>
  <div class="grid gap-2" data-pending-attachments>
    <ul v-if="model.length" class="grid gap-1">
      <li
        v-for="(item, index) in model"
        :key="`${index}-${pendingLabel(item)}`"
        class="flex min-w-0 items-center gap-2 rounded-md border border-border bg-card px-3 py-1.5 text-sm"
      >
        <Paperclip v-if="item.kind === 'file'" class="size-4 shrink-0 text-muted-foreground" />
        <Link2 v-else class="size-4 shrink-0 text-muted-foreground" />
        <span class="min-w-0 flex-1 truncate">{{ pendingLabel(item) }}</span>
        <Button type="button" variant="ghost" size="icon" :aria-label="t('actions.delete')" @click="removeAt(index)"><X class="size-4" /></Button>
      </li>
    </ul>
    <div class="flex flex-wrap gap-2">
      <Button type="button" size="sm" variant="outline" class="rounded-full" @click="picker?.click()">
        <Paperclip class="size-4" />{{ t('attachments.chooseFile') }}
      </Button>
      <Button type="button" size="sm" variant="outline" class="rounded-full" @click="linkOpen = !linkOpen">
        <Link2 class="size-4" />{{ t('attachments.link') }}
      </Button>
      <input ref="picker" type="file" multiple class="hidden" @change="addFiles" />
    </div>
    <div v-if="linkOpen" class="grid gap-1">
      <Input
        v-model="link"
        :aria-label="t('attachments.address')"
        :aria-invalid="linkBad || undefined"
        placeholder="https://…"
        @keydown.enter.prevent.stop="addLink"
        @blur="link.trim() && addLink()"
      />
      <p v-if="linkBad" class="text-xs text-destructive">{{ t('attachments.badLink') }}</p>
    </div>
  </div>
</template>
