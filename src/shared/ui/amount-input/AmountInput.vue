<script setup lang="ts">
import { computed, nextTick, ref, useAttrs, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Calculator } from 'lucide-vue-next'

import {
  GROUP_SEPARATOR,
  caretAfterSignificant,
  normalizeAmount,
  significantBefore,
} from '@/shared/lib/amount-input'
import { hasOperator } from '@/shared/lib/amount-expr'
import { activeLocale } from '@/shared/lib/locale'
import { useIsPhone } from '@/shared/lib/phone'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

import AmountKeypad from './AmountKeypad.vue'
import type { AmountExpression, AmountInputProps } from './amount-input.types'
import { useAmountExpression } from './use-amount-expression'

/**
 * A number field that groups digits as you type (`12 345,67`).
 *
 * The model is always a plain decimal string (`12345.67`) that `Amount` parses
 * as is; the grouped text exists only inside the field. Both `,` and `.` are
 * accepted as the decimal mark, and pasted amounts like `1,234.50 ₽` are read
 * correctly. The `suffix` slot sits inside the field's border (a currency);
 * `prefix` does the same on the left («1 балл =»).
 *
 * Text with an operator (`1200+350*2`) is an expression: the hint under the
 * field shows `= 1 900`, Enter or leaving the field makes it the amount (and
 * emits `expression`), Esc puts the old value back. A plain number takes the
 * old path untouched. On a phone a calculator key opens `AmountKeypad`.
 */
defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<AmountInputProps>(), {
  decimals: undefined,
  allowNegative: false,
  placeholder: '',
  disabled: false,
  id: undefined,
  size: 'default',
  calculator: true,
})
const emit = defineEmits<{ expression: [payload: AmountExpression] }>()
const model = defineModel<string | number>({ default: '' })

const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})

const decimalMark = computed(() => {
  const part = new Intl.NumberFormat(activeLocale.value).formatToParts(1.5).find((entry) => entry.type === 'decimal')
  return part?.value ?? '.'
})
const options = computed(() => ({
  decimals: props.decimals,
  allowNegative: props.allowNegative,
  decimalMark: decimalMark.value,
}))

const display = ref('')
const { t } = useI18n()
const phone = useIsPhone()
const keypadOpen = ref(false)
const calc = useAmountExpression({
  display,
  setModel: (canonical) => {
    model.value = canonical
  },
  options: () => options.value,
  emitExpression: (payload) => emit('expression', payload),
})

/** Outside changes (form reset, prefill) are reformatted; our own echo is not. */
watch(
  [() => model.value, options],
  ([value]) => {
    if (calc.expr.value !== null) return
    const text = value === null || value === undefined ? '' : String(value)
    const next = normalizeAmount(text, options.value)
    if (next.canonical !== normalizeAmount(display.value, options.value).canonical || !display.value) {
      display.value = next.display
    }
  },
  { immediate: true },
)

function onInput(event: Event) {
  const target = event.target as HTMLInputElement
  if (calc.track(target.value)) return
  const caret = target.selectionStart ?? target.value.length
  const keep = significantBefore(target.value, caret)
  const next = normalizeAmount(target.value, options.value)
  display.value = next.display
  target.value = next.display
  model.value = next.canonical
  const position = caretAfterSignificant(next.display, keep)
  void nextTick(() => target.setSelectionRange(position, position))
}

/** Backspace right after a group space deletes the digit before it, not the space. */
function onKeydown(event: KeyboardEvent) {
  if (calc.expr.value !== null && (event.key === 'Enter' || event.key === 'Escape')) {
    // Finishing or dropping an expression is not «submit the form» or «close the dialog».
    if (event.key === 'Enter') calc.commit()
    else calc.cancel()
    event.preventDefault()
    event.stopPropagation()
    return
  }
  const target = event.target as HTMLInputElement
  const start = target.selectionStart ?? 0
  if (start !== target.selectionEnd) return
  if (event.key === 'Backspace' && target.value[start - 1] === GROUP_SEPARATOR) {
    target.setSelectionRange(start - 1, start - 1)
  } else if (event.key === 'Delete' && target.value[start] === GROUP_SEPARATOR) {
    target.setSelectionRange(start + 1, start + 1)
  }
}

/** The keypad hands back a number or an expression; both end up as if typed. */
function fromKeypad(text: string) {
  if (hasOperator(text)) {
    calc.commit(text)
    return
  }
  const next = normalizeAmount(text, options.value)
  display.value = next.display
  model.value = next.canonical
}

/** On a touch screen a truncated result has no hover title: tapping it opens the keypad, which shows it in full. */
function openKeypadFromHint() {
  if (phone.value && props.calculator && !props.disabled) keypadOpen.value = true
}

const hint = calc.outcome
const hintText = computed(() => {
  if (!hint.value) return ''
  return hint.value.kind === 'result' ? `= ${hint.value.text}` : t(`amount.calc.${hint.value.code}`)
})
</script>

<template>
  <div
    :class="
      cn(
        '@container/amount flex h-9 w-full min-w-0 items-center rounded-md border border-input bg-transparent shadow-sm transition-colors focus-within:ring-1 focus-within:ring-ring has-[:disabled]:cursor-not-allowed has-[:disabled]:opacity-50',
        props.size === 'lg' && 'h-12',
        cls,
      )
    "
  >
    <span v-if="$slots.prefix" class="shrink-0 pl-3 text-sm whitespace-nowrap"><slot name="prefix" /></span>
    <input
      :id="props.id"
      :value="display"
      type="text"
      inputmode="decimal"
      autocomplete="off"
      spellcheck="false"
      :placeholder="props.placeholder"
      :disabled="props.disabled"
      :class="
        cn(
          'h-full w-0 min-w-0 flex-1 bg-transparent px-3 py-1 text-sm tabular-nums outline-none placeholder:text-muted-foreground',
          $slots.prefix && 'pl-1.5',
          props.size === 'lg' && 'text-xl font-semibold',
        )
      "
      v-bind="rest"
      @focus="calc.onFocus"
      @input="onInput"
      @keydown="onKeydown"
      @blur="calc.onBlur"
    />
    <!-- Inside the border, between the text and the suffix: it appears without moving anything around the field (rule 17). -->
    <!-- biome-ignore lint/a11y/useKeyWithClickEvents: a touch shortcut; the calculator button next to it is the keyboard path -->
    <span
      v-if="hint"
      class="max-w-[55%] shrink-0 truncate pr-2 text-xs tabular-nums"
      :class="hint.kind === 'error' ? 'text-destructive' : 'text-muted-foreground'"
      :title="hintText"
      role="status"
      data-amount-hint
      @pointerdown.prevent
      @click="openKeypadFromHint"
    >{{ hintText }}</span>
    <Button
      v-if="phone && props.calculator && !props.disabled"
      type="button"
      variant="ghost"
      size="icon-sm"
      class="hidden w-10 shrink-0 @[11rem]/amount:inline-flex"
      :aria-label="t('amount.calc.open')"
      data-amount-calc
      @click="keypadOpen = true"
    >
      <Calculator class="size-4" />
    </Button>
    <div v-if="$slots.suffix" class="flex shrink-0 items-center pr-1">
      <slot name="suffix" />
    </div>
    <AmountKeypad
      v-if="phone && props.calculator"
      v-model:open="keypadOpen"
      :text="display"
      :decimals="props.decimals"
      :allow-negative="props.allowNegative"
      :decimal-mark="decimalMark"
      @done="fromKeypad"
    />
  </div>
</template>
