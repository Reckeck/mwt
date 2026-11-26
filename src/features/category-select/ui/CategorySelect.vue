<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { categoryIcon } from '@/shared/lib/category-icons'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { PickerRow, PickerSection, PickerSheet } from '@/shared/ui/picker-sheet'

/** A category as the picker lists it. */
export interface CategoryChoice {
  value: string
  label: string
}

/**
 * The category of an operation (boards 533927, 533529): chips with the category's icon for the
 * first few (the frequent ones come first in `options`), the chosen one dark, and a dashed «Ещё»
 * that opens the sheet of board 532506 — search, the «Недавние» rows (the first few), then
 * «Все категории», every one a `PickerRow` with its icon. The chosen category always shows as a
 * chip, even when it is deeper in the list.
 * Choosing stays inside this component (rule 14).
 */
const props = withDefaults(defineProps<{ options: CategoryChoice[]; invalid?: boolean; chips?: number }>(), {
  chips: 5,
})
const model = defineModel<string>({ default: '' })
const { t } = useI18n()
const open = ref(false)
const search = ref('')

/** Options with their icon looked up once, not per render of every row. */
const choices = computed(() => props.options.map((option) => ({ ...option, icon: categoryIcon(option.value) })))
const frequent = computed(() => choices.value.slice(0, props.chips))
const shown = computed(() => {
  const picked = choices.value.find((option) => option.value === model.value)
  return picked && !frequent.value.includes(picked) ? [...frequent.value.slice(0, props.chips - 1), picked] : frequent.value
})
const found = computed(() => {
  const needle = search.value.trim().toLowerCase()
  return needle ? choices.value.filter((option) => option.label.toLowerCase().includes(needle)) : choices.value
})

const chipClass = 'h-9 gap-1.5 rounded-full px-3 text-sm font-normal [&_svg]:size-4'

function pick(value: string) {
  model.value = value
  open.value = false
  search.value = ''
}
</script>

<template>
  <div class="flex flex-wrap gap-2" data-category-chips>
    <Button
      v-for="option in shown"
      :key="option.value"
      type="button"
      :class="chipClass"
      :variant="option.value === model ? 'default' : 'outline'"
      :aria-pressed="option.value === model"
      @click="model = option.value"
    >
      <component :is="option.icon" aria-hidden="true" />
      {{ option.label }}
    </Button>
    <PickerSheet
      v-model:open="open"
      v-model:search="search"
      overlay="category-select"
      :title="t('quickAdd.category')"
      :search-label="t('quickAdd.searchCategory')"
    >
      <template #trigger>
        <Button
          id="quick-category"
          type="button"
          variant="outline"
          :class="cn(chipClass, 'border-dashed text-muted-foreground', props.invalid && 'border-destructive text-destructive')"
          :aria-invalid="props.invalid || undefined"
          :aria-label="t('quickAdd.allCategories')"
        >
          {{ t('quickAdd.moreChip') }}
        </Button>
      </template>
      <PickerSection v-if="!search.trim() && frequent.length > 0" :label="t('quickAdd.recent')">
        <PickerRow
          v-for="option in frequent"
          :key="option.value"
          :title="option.label"
          :selected="option.value === model"
          @click="pick(option.value)"
        >
          <template #icon><component :is="option.icon" aria-hidden="true" /></template>
        </PickerRow>
      </PickerSection>
      <PickerSection :label="t('quickAdd.allCategories')">
        <p v-if="found.length === 0" class="py-6 text-center text-sm text-muted-foreground">{{ t('quickAdd.noCategory') }}</p>
        <PickerRow
          v-for="option in found"
          :key="option.value"
          :title="option.label"
          :selected="option.value === model"
          @click="pick(option.value)"
        >
          <template #icon><component :is="option.icon" aria-hidden="true" /></template>
        </PickerRow>
      </PickerSection>
    </PickerSheet>
  </div>
</template>
