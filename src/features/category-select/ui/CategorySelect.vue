<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'

import { categoryIcon } from '@/shared/lib/category-icons'
import { cn } from '@/shared/lib/utils'
import { Button } from '@/shared/ui/button'
import { PickerSheet } from '@/shared/ui/picker-sheet'

/** A category as the picker lists it. */
export interface CategoryChoice {
  value: string
  label: string
}

/**
 * The category of an operation (boards 533927, 533529): chips with the category's icon for the
 * first few (the frequent ones come first in `options`), the chosen one dark, and a dashed «Ещё»
 * that opens the sheet of board 532506 — search, «Частые» chips, then every category as a row with
 * its icon. The chosen category always shows as a chip, even when it is deeper in the list.
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
      <template v-if="!search.trim() && frequent.length > 0">
        <h3 class="px-1 pb-2 text-xs text-muted-foreground">{{ t('quickAdd.frequent') }}</h3>
        <div class="flex flex-wrap gap-2 pb-4">
          <Button
            v-for="option in frequent"
            :key="option.value"
            type="button"
            :class="chipClass"
            :variant="option.value === model ? 'default' : 'outline'"
            :aria-pressed="option.value === model"
            @click="pick(option.value)"
          >
            <component :is="option.icon" aria-hidden="true" />
            {{ option.label }}
          </Button>
        </div>
      </template>
      <h3 class="px-1 pb-1 text-xs text-muted-foreground">{{ t('quickAdd.allCategories') }}</h3>
      <p v-if="found.length === 0" class="py-6 text-center text-sm text-muted-foreground">{{ t('quickAdd.noCategory') }}</p>
      <Button
        v-for="option in found"
        :key="option.value"
        type="button"
        variant="ghost"
        class="h-auto w-full justify-start gap-3 px-1 py-2 text-left text-base font-medium"
        :class="option.value === model && 'bg-accent'"
        :aria-pressed="option.value === model"
        @click="pick(option.value)"
      >
        <span class="grid size-10 shrink-0 place-items-center rounded-full bg-muted [&_svg]:size-5">
          <component :is="option.icon" aria-hidden="true" />
        </span>
        {{ option.label }}
      </Button>
    </PickerSheet>
  </div>
</template>
