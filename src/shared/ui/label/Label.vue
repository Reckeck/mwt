<script setup lang="ts">
import { computed, useAttrs } from 'vue'
import { useI18n } from 'vue-i18n'

import { cn } from '@/shared/lib/utils'

import type { LabelProps } from './label.types'

defineOptions({ inheritAttrs: false })
const props = withDefaults(defineProps<LabelProps>(), { required: false })

const { t } = useI18n()
const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})
</script>

<template>
  <!-- biome-ignore lint/a11y/noLabelWithoutControl: primitive forwards `for` from $attrs to the caller. -->
  <label
    :class="cn('text-sm font-medium leading-none peer-disabled:cursor-not-allowed peer-disabled:opacity-70', cls)"
    v-bind="rest"
  >
    <slot />
    <span v-if="props.required" class="ml-0.5 text-destructive" :title="t('validation.requiredMark')" aria-hidden="true">*</span>
    <span v-if="props.required" class="sr-only">{{ t('validation.requiredMark') }}</span>
  </label>
</template>
