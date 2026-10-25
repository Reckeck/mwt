<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { useIsPhone } from '@/shared/lib/phone'

import type { FormDialogProps } from './form-dialog.types'

/**
 * A dialog whose body is one form: Enter saves, the save button stays enabled
 * (only a save in flight disables it, with a spinner), Cancel closes.
 *
 * The caller checks the fields: `@submit="form.submit(save.mutate, $event)"`
 * with `useFormValidation`, so a failed save points at the field to fix.
 * Extra buttons on the left of the footer go in `#footer-start`.
 */
const props = withDefaults(defineProps<FormDialogProps>(), {
  open: undefined,
  description: undefined,
  size: 'md',
  submitLabel: undefined,
  pending: false,
  phoneSubmitOnly: false,
  overlay: undefined,
})
const emit = defineEmits<{ 'update:open': [boolean]; submit: [Event] }>()

const { t } = useI18n()
const phone = useIsPhone()
</script>

<template>
  <Dialog
    :open="props.open"
    :title="props.title"
    :description="props.description"
    :size="props.size"
    :overlay="props.overlay"
    @update:open="(value) => emit('update:open', value)"
  >
    <form :id="props.formId" class="@container flex flex-col gap-4" @submit.prevent="emit('submit', $event)">
      <slot />
    </form>

    <template #footer>
      <div class="mr-auto flex items-center gap-2">
        <slot name="footer-start" />
      </div>
      <Button v-if="!(props.phoneSubmitOnly && phone)" variant="ghost" type="button" @click="emit('update:open', false)">{{ t('actions.cancel') }}</Button>
      <Button
        :form="props.formId"
        type="submit"
        :class="props.phoneSubmitOnly && phone && 'h-11 w-full'"
        :disabled="props.pending"
        :loading="props.pending"
      >
        {{ props.submitLabel ?? t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
