<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { cn } from '@/shared/lib/utils'

/**
 * Where a wizard is (Money 1 · 3–4): a segment per step, the done ones filled,
 * and `Step 1 of 2 · For what?` under them. Steps are numbered from 1.
 */
const props = defineProps<{ step: number; titles: string[] }>()
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-2">
    <div class="flex gap-1.5" aria-hidden="true">
      <span
        v-for="(_, at) in props.titles"
        :key="at"
        :class="cn('h-1 flex-1 rounded-full transition-colors', at < props.step ? 'bg-primary' : 'bg-muted')"
      />
    </div>
    <p class="text-sm text-muted-foreground" aria-live="polite">
      {{ t('wizard.step', { step: props.step, total: props.titles.length, title: props.titles[props.step - 1] }) }}
    </p>
  </div>
</template>
