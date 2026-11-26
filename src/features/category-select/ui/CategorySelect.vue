<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronDown } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { PickerSheet } from '@/shared/ui/picker-sheet'

/** A category as the picker lists it. */
export interface CategoryChoice {
  value: string
  label: string
}

/**
 * The category of an operation: a row of chips for the first few (the frequent ones come first in
 * `options`) and a «more» chip that opens a bottom sheet with a search field and the full flat
 * list. The chosen category always shows as a lit chip, even when it is deeper in the list.
 * Choosing stays inside this component (rule 14).
 */
const props = withDefaults(defineProps<{ options: CategoryChoice[]; invalid?: boolean; placeholder?: string; chips?: number }>(), {
  chips: 5,
  placeholder: undefined,
})
const model = defineModel<string>({ default: '' })
const { t } = useI18n()
const open = ref(false)
const search = ref('')

const shown = computed(() => {
  const head = props.options.slice(0, props.chips)
  const picked = props.options.find((option) => option.value === model.value)
  return picked && !head.includes(picked) ? [...head.slice(0, props.chips - 1), picked] : head
})
const found = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return needle ? props.options.filter((option) => option.label.toLowerCase().includes(needle)) : props.options
})

function pick(value: string) {
  model.value = value
  open.value = false
  search.value = ''
}
</script>

<template>
  <div class="flex flex-wrap gap-2">
    <Button
      v-for="option in shown"
      :key="option.value"
      type="button"
      size="sm"
      class="rounded-full"
      :variant="option.value === model ? 'default' : 'outline'"
      :aria-pressed="option.value === model"
      @click="model = option.value"
    >
      {{ option.label }}
    </Button>
    <PickerSheet v-model:open="open" v-model:search="search" overlay="category-select" :title="t('quickAdd.category')" :search-label="t('quickAdd.searchCategory')">
      <template #trigger>
        <Button
          id="quick-category"
          type="button"
          size="sm"
          variant="outline"
          class="gap-1 rounded-full"
          :aria-invalid="props.invalid || undefined"
          :class="props.invalid && 'border-destructive'"
        >
          {{ props.placeholder ?? t('quickAdd.allCategories') }}
          <ChevronDown class="size-4 text-muted-foreground" />
        </Button>
      </template>
      <h3 class="px-1 pb-1 text-xs font-medium text-muted-foreground">{{ t('quickAdd.allCategories') }}</h3>
      <p v-if="found.length === 0" class="py-6 text-center text-sm text-muted-foreground">{{ t('quickAdd.noCategory') }}</p>
      <Button
        v-for="option in found"
        :key="option.value"
        type="button"
        variant="ghost"
        class="h-auto w-full justify-start px-2 py-3 text-left text-base font-normal"
        :class="option.value === model && 'bg-accent'"
        :aria-pressed="option.value === model"
        @click="pick(option.value)"
      >
        {{ option.label }}
      </Button>
    </PickerSheet>
  </div>
</template>
