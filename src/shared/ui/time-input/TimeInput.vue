<script setup lang="ts">
import { ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'

import { cn } from '@/shared/lib/utils'
import { Input } from '@/shared/ui/input'

import type { TimeInputProps } from './time-input.types'
import { maskTime, parseTime } from './time-value'

/**
 * A time of day, always 24 hours («18:30»): the browser's `type="time"`
 * follows the system and shows AM/PM. The model is the string such an input
 * holds («HH:MM», or `''`), so it drops in for one. Leaving the field
 * finishes the time («9» → 09:00); what is not a time goes back to the last
 * good value.
 */
defineOptions({ inheritAttrs: false })
const props = defineProps<TimeInputProps>()
const model = defineModel<string>({ default: '' })

const { t } = useI18n()
const text = ref(model.value)
watch(model, (next) => {
  if (parseTime(text.value) !== next) text.value = next
})

function onInput(next: string | number) {
  text.value = maskTime(String(next))
  const time = parseTime(text.value)
  // A complete «HH:MM» goes out at once; shorter waits for blur.
  if (time && text.value.length === 5) model.value = time
}

function finish() {
  if (!text.value.trim()) {
    model.value = ''
    return
  }
  const time = parseTime(text.value)
  if (time) model.value = time
  text.value = time ?? model.value
}
</script>

<template>
  <Input
    :id="props.id"
    v-bind="$attrs"
    :model-value="text"
    :disabled="props.disabled"
    :aria-invalid="props.invalid || undefined"
    :class="cn('tabular-nums', props.invalid && 'border-destructive', $attrs.class as string)"
    inputmode="numeric"
    autocomplete="off"
    maxlength="5"
    :placeholder="t('ui.timePlaceholder')"
    @update:model-value="onInput"
    @blur="finish"
    @keydown.enter="finish"
  />
</template>
