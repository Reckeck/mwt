<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Paperclip } from 'lucide-vue-next'

import { AttachmentAdd, type LinkDraft } from '@/entities/attachment'
import { Kbd } from '@/shared/ui/kbd'
import { useCoarsePointer } from '@/shared/lib/phone'

import type { AttachmentsBlockProps } from '../../attachment-edit.types'
import { useAttachmentCatch } from '../../model/use-attachment-catch'
import { useAttachmentIntake } from '../../model/use-attachment-intake'
import { useAttachments } from '../../model/use-attachments'
import AttachmentDropOverlay from './AttachmentDropOverlay.vue'
import AttachmentIntakeRows from './AttachmentIntakeRows.vue'
import AttachmentItem from './AttachmentItem.vue'

/**
 * `Files and links · N` (Attach 1–4, 7): receipts, contracts, screenshots and
 * links right in the operation. Added from `Add`, by dragging a file onto
 * the card or with Ctrl V; deleted at once, with `Undo` in the toast.
 */
const props = withDefaults(defineProps<AttachmentsBlockProps>(), { dropTarget: null, active: true })

const { t } = useI18n()
const coarse = useCoarsePointer()
const root = ref<HTMLElement | null>(null)

const store = useAttachments(() => props.owner, () => props.active)
const items = computed(() => store.list.data.value ?? [])
const intake = useAttachmentIntake((vars) => store.addFile.mutateAsync(vars))

const linkOpen = ref(false)
const linkUrl = ref('')
function openLink(url = '') {
  linkUrl.value = url
  linkOpen.value = true
}
function addLink(draft: LinkDraft) {
  store.addLink.mutate(draft, { onSuccess: () => (linkOpen.value = false) })
}

const { dragging } = useAttachmentCatch(
  () => props.dropTarget ?? root.value,
  () => props.active,
  { files: intake.takeAll, link: openLink },
)

const count = computed(() => items.value.length + intake.pending.value.length)
const empty = computed(() => !count.value && !intake.failed.value.length)
</script>

<template>
  <section ref="root" class="relative grid gap-2 border-t pt-4" data-tour="tx-attachments" data-attachments>
    <div class="flex items-center gap-2">
      <h3 class="flex-1 text-sm font-semibold">
        {{ count ? t('attachments.titleCount', { count }) : t('attachments.title') }}
      </h3>
      <AttachmentAdd
        v-model:link-open="linkOpen"
        :link-url="linkUrl"
        :pending="store.addLink.isPending.value"
        @files="intake.takeAll"
        @link="addLink"
      />
    </div>

    <div v-if="!empty" class="grid">
      <AttachmentItem
        v-for="item in items"
        :key="item.id"
        :item="item"
        @open="store.open(item)"
        @save="store.saveAs.mutate(item)"
        @rename="(title) => store.rename.mutate({ id: item.id, title })"
        @remove="store.remove.mutate(item)"
      />
      <AttachmentIntakeRows
        :pending="intake.pending.value"
        :failed="intake.failed.value"
        @cancel="intake.cancel"
        @dismiss="intake.dismiss"
      />
    </div>

    <div
      v-if="empty && store.list.isSuccess.value"
      class="flex items-start gap-3 rounded-lg border border-dashed px-4 py-3.5 text-sm text-muted-foreground"
      data-attachments-empty
    >
      <Paperclip class="mt-0.5 size-4 shrink-0" />
      <p>
        {{ t('attachments.emptyWhat') }}
        <template v-if="!coarse">{{ t('attachments.emptyHow') }} <Kbd>Ctrl V</Kbd>.</template>
      </p>
    </div>
    <p v-else-if="!empty && !coarse" class="text-xs text-muted-foreground">
      {{ t('attachments.dropHint') }} <Kbd>Ctrl V</Kbd>
    </p>

    <Teleport v-if="dragging" :to="props.dropTarget ?? root ?? 'body'">
      <AttachmentDropOverlay :owner-label="props.ownerLabel" />
    </Teleport>
  </section>
</template>
