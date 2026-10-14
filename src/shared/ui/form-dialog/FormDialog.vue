<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Spinner } from '@/shared/ui/spinner'

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
})
const emit = defineEmits<{ 'update:open': [boolean]; submit: [Event] }>()

const { t } = useI18n()
</script>

<template>
  <Dialog
    :open="props.open"
    :title="props.title"
    :description="props.description"
    :size="props.size"
    @update:open="(value) => emit('update:open', value)"
  >
    <form :id="props.formId" class="@container flex flex-col gap-4" @submit.prevent="emit('submit', $event)">
      <slot />
    </form>

    <template #footer>
      <div class="mr-auto flex items-center gap-2">
        <slot name="footer-start" />
      </div>
      <Button variant="ghost" type="button" @click="emit('update:open', false)">{{ t('actions.cancel') }}</Button>
      <Button :form="props.formId" type="submit" :disabled="props.pending">
        <Spinner v-if="props.pending" />
        {{ props.submitLabel ?? t('actions.save') }}
      </Button>
    </template>
  </Dialog>
</template>
