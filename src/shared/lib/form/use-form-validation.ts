import { type MaybeRefOrGetter, type WatchSource, computed, nextTick, ref, toValue, watch } from 'vue'

import type { FieldError } from './validators'

export interface FormValidationOptions {
  /** Forget a failed attempt when this changes — e.g. the dialog's `open` or the wizard step. */
  resetOn?: WatchSource | WatchSource[]
}

/**
 * The one validation flow of every form (AGENTS.md, "Forms"):
 *
 * - the save button stays enabled (only a pending save disables it);
 * - pressing it with a missing or wrong field shows the message under each
 *   such field, puts the focus on the first one and does not save;
 * - from then on the messages follow the input, so a fixed field clears itself.
 *
 * `rules` maps a field name to its current error (`null` = fine); build it with
 * the checks from `validators.ts`. A field rendered with `FormField` and its
 * `error` set to `form.error('name')` gets the message and the red border.
 */
export function useFormValidation<Field extends string>(
  rules: MaybeRefOrGetter<Partial<Record<Field, FieldError>>>,
  options: FormValidationOptions = {},
) {
  const attempted = ref(false)

  const errors = computed(() => toValue(rules))
  const valid = computed(() => Object.values(errors.value).every((error) => !error))

  /** The message to show under a field — only after a save was tried. */
  function error(field: Field): string | undefined {
    return attempted.value ? errors.value[field] || undefined : undefined
  }

  /** The first message, for a one-line form that shows a single line under its row. */
  function firstMessage(): string | undefined {
    return attempted.value ? (Object.values(errors.value).find(Boolean) as string | undefined) : undefined
  }

  /** `data-invalid` for a control's wrapper in a form without `FormField` (an inline row). */
  function invalidAttr(field: Field): '' | undefined {
    return error(field) ? '' : undefined
  }

  function reset() {
    attempted.value = false
  }
  if (options.resetOn) watch(options.resetOn, reset)

  async function focusFirstInvalid(root: ParentNode) {
    await nextTick()
    const field = root.querySelector<HTMLElement>('[data-invalid]')
    const control = field?.querySelector<HTMLElement>('input, select, textarea, button, [tabindex]') ?? field
    control?.focus()
    field?.scrollIntoView({ block: 'nearest' })
  }

  /**
   * Wrap the save action: `@submit.prevent="form.submit(save)"` on a `<form>`,
   * or `@click="form.submit(save, $event)"` on a button outside one.
   */
  function submit(action: () => unknown, event?: Event): boolean {
    attempted.value = true
    if (valid.value) {
      action()
      return true
    }
    const target = event?.target instanceof Element ? event.target : null
    const root = target?.closest('form, [role="dialog"]') ?? document
    void focusFirstInvalid(root)
    return false
  }

  return { errors, valid, attempted, error, firstMessage, invalidAttr, submit, reset }
}

export type FormValidation<Field extends string = string> = ReturnType<typeof useFormValidation<Field>>
