<script setup lang="ts">
import { Label } from '@/shared/ui/label'

import type { FormFieldProps } from './form-field.types'

/**
 * Label, control and its message, the same in every form: the `*` of a
 * required field, the error under it after a failed save (with a red border on
 * the control, see `[data-invalid]` in `styles.css`), the hint otherwise.
 */
const props = withDefaults(defineProps<FormFieldProps>(), {
  label: '',
  for: undefined,
  required: false,
  error: undefined,
  hint: undefined,
})
</script>

<template>
  <div class="grid content-start gap-1.5" :data-invalid="props.error ? '' : undefined">
    <Label v-if="props.label" :for="props.for" :required="props.required">{{ props.label }}</Label>
    <slot name="label" />
    <slot />
    <p v-if="props.error" class="text-xs text-destructive" role="alert">{{ props.error }}</p>
    <p v-else-if="props.hint" class="text-xs text-muted-foreground">{{ props.hint }}</p>
  </div>
</template>
