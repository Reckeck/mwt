<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronLeft } from 'lucide-vue-next'

import { formatDateRange } from '@/shared/lib/date-range'

/** «‹ Шашлыки · 2–4 ноя»: the way back to Money and what the event is called. */
const props = defineProps<{ event: SharedEvent | null }>()
const { t, locale } = useI18n()

const BACK_CLASS = [
  'flex size-10 shrink-0 items-center justify-center rounded-md text-muted-foreground hover:bg-accent',
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring',
]

const dates = computed(() =>
  props.event ? formatDateRange({ start: props.event.starts_on, end: props.event.ends_on }, locale.value) : '',
)
</script>

<template>
  <header class="flex min-w-0 items-center gap-2">
    <RouterLink
      :to="{ name: 'money' }"
      :class="BACK_CLASS"
      :aria-label="t('actions.back')"
    >
      <ChevronLeft class="size-5" />
    </RouterLink>
    <h1 v-if="props.event" class="min-w-0 truncate text-xl font-semibold">
      {{ props.event.title }}<template v-if="dates"> · {{ dates }}</template>
    </h1>
  </header>
</template>
