<script setup lang="ts">
import { nextTick, onMounted, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { required, useFormValidation } from '@/shared/lib/form'

import type { LinkDraft } from '../../attachment.types'
import { looksLikeLink } from '../../lib/read-file'

/**
 * A link (Attach 4): the address, required, and a caption — empty shows the
 * address. Enter adds, Esc closes. An address in the clipboard is filled in.
 */
const props = defineProps<{ url?: string; pending?: boolean }>()
const emit = defineEmits<{ submit: [draft: LinkDraft]; cancel: [] }>()

const { t } = useI18n()
const url = ref(props.url ?? '')
const title = ref('')

const form = useFormValidation(() => ({
  url: required(url.value) ?? (looksLikeLink(url.value) ? null : t('attachments.badLink')),
}))

const urlInput = ref<InstanceType<typeof Input> | null>(null)
onMounted(async () => {
  if (!url.value) {
    const text = await navigator.clipboard?.readText?.().catch(() => '')
    if (text && looksLikeLink(text)) url.value = text.trim()
  }
  void nextTick(() => (urlInput.value?.$el as HTMLInputElement | undefined)?.focus())
})
</script>

<template>
  <form
    class="grid gap-3"
    data-attachment-link-form
    @submit.prevent="form.submit(() => emit('submit', { url: url.trim(), title: title.trim() }), $event)"
    @keydown.esc.stop.prevent="emit('cancel')"
  >
    <b class="text-sm font-semibold pointer-coarse:text-base">{{ t('attachments.link') }}</b>
    <FormField for="attachment-link-url" :label="t('attachments.address')" required :error="form.error('url')">
      <Input
        id="attachment-link-url"
        ref="urlInput"
        v-model="url"
        inputmode="url"
        autocomplete="off"
        spellcheck="false"
        placeholder="https://"
      />
    </FormField>
    <FormField for="attachment-link-title" :label="t('attachments.caption')" :hint="t('attachments.captionHint')">
      <Input id="attachment-link-title" v-model="title" autocomplete="off" />
    </FormField>
    <div class="flex items-center justify-end gap-2 pointer-coarse:-mx-4 pointer-coarse:mt-1 pointer-coarse:border-t pointer-coarse:px-4 pointer-coarse:pt-3">
      <Button type="button" variant="outline" size="sm" class="pointer-coarse:h-12 pointer-coarse:px-4 pointer-coarse:text-sm" @click="emit('cancel')">
        {{ t('actions.cancel') }}
      </Button>
      <Button type="submit" size="sm" class="pointer-coarse:h-12 pointer-coarse:flex-1 pointer-coarse:text-sm" :loading="props.pending" :disabled="props.pending">
        {{ t('actions.add') }}
      </Button>
    </div>
  </form>
</template>
