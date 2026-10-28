<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Delete } from 'lucide-vue-next'

import { compactExpr, evaluateAmountExpr, hasOperator } from '@/shared/lib/amount-expr'
import { normalizeAmount } from '@/shared/lib/amount-input'
import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerDescription, DrawerHeader, DrawerTitle } from '@/shared/ui/drawer'

/**
 * The phone's calculator for an amount field: a bottom sheet with digits and
 * `+ − × ÷ ( ) %`. It edits its own text; «Готово» hands it back and the field
 * applies it like a typed expression. «Назад» closes it (a Reka layer).
 */
const props = defineProps<{ text: string; decimals?: number; allowNegative?: boolean; decimalMark: string }>()
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ done: [text: string] }>()
const { t } = useI18n()

const typed = ref('')
watch(
  open,
  (value) => {
    // The field's own digits come back without their group spaces.
    if (value) typed.value = props.text.replace(/[\s\u00a0\u202f]/g, '')
  },
  { immediate: true },
)

const KEYS = [
  ['C', '(', ')', 'back'],
  ['7', '8', '9', '÷'],
  ['4', '5', '6', '×'],
  ['1', '2', '3', '−'],
  ['0', 'mark', '%', '+'],
] as const

const evaluated = computed(() => {
  if (!typed.value || !hasOperator(typed.value)) return null
  return evaluateAmountExpr(typed.value, { decimals: props.decimals })
})
const opts = computed(() => ({
  decimals: props.decimals,
  allowNegative: props.allowNegative,
  decimalMark: props.decimalMark,
}))
const resultText = computed(() => {
  if (evaluated.value?.ok) return `= ${normalizeAmount(evaluated.value.value, opts.value).display}`
  return ''
})
const errorText = computed(() => {
  const outcome = evaluated.value
  if (!outcome || outcome.ok) return ''
  return outcome.code === 'divByZero' ? t('amount.calc.divByZero') : ''
})
/** What «Готово» would hand back is shown as the field's own number once there is no operator. */
const shown = computed(() => (typed.value ? typed.value.replace(/\./g, props.decimalMark) : '0'))
const failed = ref(false)

function press(key: string) {
  failed.value = false
  if (key === 'C') typed.value = ''
  else if (key === 'back') typed.value = typed.value.slice(0, -1)
  else if (key === 'mark') typed.value += props.decimalMark
  else typed.value += key
}

function equals() {
  const outcome = evaluated.value
  if (!outcome?.ok) {
    failed.value = Boolean(typed.value)
    return
  }
  typed.value = outcome.value
}

function done() {
  if (!typed.value) {
    open.value = false
    return
  }
  const text = hasOperator(typed.value) ? compactExpr(typed.value) : typed.value
  if (hasOperator(text) && !evaluateAmountExpr(text, { decimals: props.decimals }).ok) {
    failed.value = true
    return
  }
  emit('done', text)
  open.value = false
}
</script>

<template>
  <Drawer v-model:open="open">
    <DrawerContent layer="amount-keypad" data-amount-keypad>
      <DrawerHeader class="sr-only">
        <DrawerTitle>{{ t('amount.calc.title') }}</DrawerTitle>
        <DrawerDescription>{{ t('amount.calc.hint') }}</DrawerDescription>
      </DrawerHeader>
      <div class="px-4 pt-2">
        <div class="rounded-lg border bg-background px-3 py-2 text-right">
          <div class="min-h-8 overflow-x-auto text-2xl font-semibold tabular-nums whitespace-nowrap" data-keypad-expr>
            {{ shown }}
          </div>
          <div
            class="min-h-5 text-sm tabular-nums"
            :class="errorText || failed ? 'text-destructive' : 'text-muted-foreground'"
            data-keypad-result
          >
            {{ errorText || (failed ? t('amount.calc.syntax') : resultText) }}
          </div>
        </div>
        <div class="mt-3 grid grid-cols-4 gap-2">
          <template v-for="row in KEYS" :key="row.join('')">
            <Button
              v-for="key in row"
              :key="key"
              type="button"
              :variant="'÷×−+%'.includes(key) || key === 'C' ? 'secondary' : 'outline'"
              class="h-12 text-lg"
              :aria-label="key === 'back' ? t('amount.calc.back') : key === 'mark' ? props.decimalMark : key"
              :data-calc-key="key"
              @click="press(key)"
            >
              <Delete v-if="key === 'back'" class="size-5" />
              <template v-else-if="key === 'mark'">{{ props.decimalMark }}</template>
              <template v-else>{{ key }}</template>
            </Button>
          </template>
        </div>
        <div class="mt-2 grid grid-cols-4 gap-2 pb-4">
          <Button type="button" variant="outline" class="h-12 text-lg" data-calc-key="=" @click="equals">=</Button>
          <Button type="button" class="col-span-3 h-12" data-calc-done @click="done">{{ t('amount.calc.done') }}</Button>
        </div>
      </div>
    </DrawerContent>
  </Drawer>
</template>
