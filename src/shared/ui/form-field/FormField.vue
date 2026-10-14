<script setup lang="ts">
import { Field, FieldDescription, FieldError } from '@/shared/ui/field'
import { Label } from '@/shared/ui/label'

import type { FormFieldProps } from './form-field.types'

/**
 * Label, control and its message, the same in every form: the `*` of a
 * required field, the error under it after a failed save (with a red border on
 * the control, see `[data-invalid]` in `styles.css`), the hint otherwise.
 *
 * Built on shadcn `Field`, so `FieldGroup`/`FieldSet` around it lay out as the
 * registry expects; the project API (`label`, `required`, `error`, `hint`) stays.
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
  <Field class="content-start gap-1.5" :data-invalid="props.error ? 'true' : undefined">
    <Label v-if="props.label" :for="props.for" :required="props.required">{{ props.label }}</Label>
    <slot name="label" />
    <slot />
    <FieldError v-if="props.error" class="text-xs">{{ props.error }}</FieldError>
    <FieldDescription v-else-if="props.hint" class="text-xs">{{ props.hint }}</FieldDescription>
  </Field>
</template>
