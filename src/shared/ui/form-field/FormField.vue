<script setup lang="ts">
import { computed, onMounted, onUpdated, useId, useSlots, useTemplateRef, watch } from 'vue'

import { Field, FieldDescription, FieldError } from '@/shared/ui/field'
import { Label } from '@/shared/ui/label'

import type { FormFieldProps } from './form-field.types'

/**
 * Label, control and its message, the same in every form: the `*` of a
 * required field, the error under it after a failed save (with a red border on
 * the control, see `[data-invalid]` in `styles.css`), the hint otherwise.
 *
 * Built on shadcn `Field`, so `FieldGroup`/`FieldSet` around it lay out as the
 * registry expects; the project API (`label`, `required`, `error`, `hint`) stays. A hint with a sum
 * comes in the `hint` slot, so the sum is a `<Money>`.
 */
const props = withDefaults(defineProps<FormFieldProps>(), {
  label: '',
  for: undefined,
  required: false,
  error: undefined,
  hint: undefined,
})

/**
 * The control comes from the slot, so its error and hint are linked after the
 * render: `aria-invalid` and `aria-describedby` on it, pointing at the message
 * under it — a screen reader reads «Сумма, обязательно, введите сумму».
 */
const slots = useSlots()
const hasHint = computed(() => Boolean(props.hint) || Boolean(slots.hint))
const messageId = useId()
const root = useTemplateRef<{ $el: HTMLElement }>('root')
const CONTROL = 'input:not([type="hidden"]), textarea, select, [role="combobox"], button[aria-haspopup]'

function link() {
  const box = root.value?.$el
  if (!box) return
  const control = (props.for && box.querySelector(`#${CSS.escape(props.for)}`)) || box.querySelector(CONTROL)
  if (!control) return
  if (props.error) control.setAttribute('aria-invalid', 'true')
  else control.removeAttribute('aria-invalid')
  if (props.error || hasHint.value) control.setAttribute('aria-describedby', messageId)
  else if (control.getAttribute('aria-describedby') === messageId) control.removeAttribute('aria-describedby')
}
onMounted(link)
onUpdated(link)
watch(() => [props.error, props.hint], link, { flush: 'post' })
</script>

<template>
  <Field ref="root" class="content-start gap-1.5" :data-invalid="props.error ? 'true' : undefined">
    <Label v-if="props.label" :for="props.for" :required="props.required">{{ props.label }}</Label>
    <slot name="label" />
    <slot />
    <FieldError v-if="props.error" :id="messageId" class="text-xs">{{ props.error }}</FieldError>
    <FieldDescription v-else-if="hasHint" :id="messageId" class="text-xs"><slot name="hint">{{ props.hint }}</slot></FieldDescription>
  </Field>
</template>
