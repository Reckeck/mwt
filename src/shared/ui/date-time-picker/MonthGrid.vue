<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronLeft, ChevronRight } from 'lucide-vue-next'

import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'

/** Twelve months of a year to pick one, for the `month` mode (the calendar shows days). */
const props = defineProps<{ value: string; locale: string; min?: string; max?: string }>()
const emit = defineEmits<{ pick: [value: string] }>()
const { t } = useI18n()

const picked = computed(() => /^(\d{4})-(\d{2})/.exec(props.value))
const year = ref(picked.value ? Number(picked.value[1]) : new Date().getFullYear())
const names = computed(() =>
  Array.from({ length: 12 }, (_unused, index) =>
    new Intl.DateTimeFormat(props.locale, { month: 'short' }).format(new Date(2000, index, 1)),
  ),
)
const keyOf = (month: number) => `${year.value}-${String(month).padStart(2, '0')}`
const outside = (key: string) => Boolean((props.min && key < props.min.slice(0, 7)) || (props.max && key > props.max.slice(0, 7)))
</script>

<template>
  <div class="grid w-[min(16rem,calc(100vw-2rem))] gap-2 p-3">
    <div class="flex items-center justify-between">
      <Button variant="ghost" size="icon-sm" :aria-label="t('ui.prevYear')" @click="year -= 1">
        <ChevronLeft class="size-4" />
      </Button>
      <span class="text-sm font-medium tabular-nums">{{ year }}</span>
      <Button variant="ghost" size="icon-sm" :aria-label="t('ui.nextYear')" @click="year += 1">
        <ChevronRight class="size-4" />
      </Button>
    </div>
    <div class="grid grid-cols-3 gap-1">
      <Button
        v-for="(name, index) in names"
        :key="name"
        :variant="props.value.startsWith(keyOf(index + 1)) ? 'default' : 'ghost'"
        size="sm"
        :disabled="outside(keyOf(index + 1))"
        :class="cn('capitalize', !props.value.startsWith(keyOf(index + 1)) && 'font-normal')"
        :aria-pressed="props.value.startsWith(keyOf(index + 1))"
        @click="emit('pick', keyOf(index + 1))"
      >
        {{ name }}
      </Button>
    </div>
  </div>
</template>
