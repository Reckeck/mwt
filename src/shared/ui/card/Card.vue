<script setup lang="ts">
import { computed, useAttrs } from 'vue'

import { cn } from '@/shared/lib/utils'

defineOptions({ inheritAttrs: false })
const attrs = useAttrs()
const cls = computed(() => (attrs as Record<string, unknown>).class as string | undefined)
const rest = computed(() => {
  const { class: _cls, ...others } = attrs as Record<string, unknown>
  return others
})
</script>

<template>
  <div :class="cn('rounded-xl border border-border bg-card text-card-foreground shadow-sm', cls)" v-bind="rest">
    <div v-if="$slots.header" class="flex flex-col gap-1.5 p-6 pb-4">
      <slot name="header" />
    </div>
    <!-- Only the section right under another one drops its top padding;
         a card without a header used to have its content glued to the edge. -->
    <div v-if="$slots.default" :class="cn('p-6', $slots.header && 'pt-0')">
      <slot />
    </div>
    <div
      v-if="$slots.footer"
      :class="cn('flex items-center gap-2 p-6', ($slots.header || $slots.default) && 'pt-0')"
    >
      <slot name="footer" />
    </div>
  </div>
</template>
