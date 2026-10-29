<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Copy, Share2 } from 'lucide-vue-next'

import { useSharedEventShareText } from '@/entities/shared-event'
import { copyText } from '@/shared/lib/clipboard'
import { notify } from '@/shared/lib/notify'
import { useIsPhone } from '@/shared/lib/phone'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { SectionState } from '@/shared/ui/section-state'

import type { SharedEventShareProps } from '../shared-event-share.types'

/**
 * `Share the settlement`: the text for the event's chat — purchases, shares, who pays whom and the
 * organizer's requisites — read-only. A computer copies it to the clipboard; a phone also offers
 * the system `Share` when there is one.
 */
const props = defineProps<SharedEventShareProps>()
const open = defineModel<boolean>('open', { required: true })

const { t, locale } = useI18n()
const phone = useIsPhone()

const text = useSharedEventShareText(
  () => props.eventId,
  () => locale.value,
  () => open.value,
)
const body = computed(() => text.data.value ?? '')
const canShare = computed(() => phone.value && typeof navigator.share === 'function')

async function share() {
  try {
    await navigator.share({ title: props.title, text: body.value })
  } catch (err) {
    // Closing the system sheet is not an error.
    if (!(err instanceof DOMException && err.name === 'AbortError')) notify.error(err, 'sharedEvents.share.failed')
  }
}
</script>

<template>
  <Dialog
    v-model:open="open"
    overlay="shared-share"
    size="form"
    :title="t('sharedEvents.share.title')"
    :description="t('sharedEvents.share.description')"
  >
    <SectionState
      :loading="text.isPending.value"
      :error="text.error.value"
      skeleton="report"
      compact
      @retry="text.refetch()"
    >
      <pre
        class="min-h-24 whitespace-pre-wrap break-words rounded-md border border-border bg-muted/50 p-3 font-mono text-sm leading-relaxed"
        >{{ body }}</pre
      >
    </SectionState>
    <template #footer>
      <Button v-if="canShare" variant="outline" class="max-md:h-11 max-md:flex-1" :disabled="!body" @click="share">
        <Share2 class="size-4" />
        {{ t('sharedEvents.share.share') }}
      </Button>
      <Button class="max-md:h-11 max-md:flex-1" :disabled="!body" @click="copyText(body, 'sharedEvents.share.copied')">
        <Copy class="size-4" />
        {{ t('sharedEvents.share.copy') }}
      </Button>
    </template>
  </Dialog>
</template>
