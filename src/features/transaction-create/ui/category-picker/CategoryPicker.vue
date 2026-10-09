<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { categoryLabel } from '@/shared/lib/categories'

import type { CategoryPickerProps } from './category-picker.types'

const props = defineProps<CategoryPickerProps>()

/** Picked category keys. */
const picked = defineModel<string[]>({ required: true })

const { t } = useI18n()

const categories = useQuery({
  queryKey: ['categories'],
  queryFn: () => call<Category[]>('list_categories'),
})

/** Offered groups in order, each with its live categories (and any already picked). */
const sections = computed(() =>
  props.groups
    .map((group) => ({
      group,
      items: (categories.data.value ?? []).filter(
        (category) => category.group === group && (!category.archived || picked.value.includes(category.key)),
      ),
    }))
    .filter((section) => section.items.length),
)

function toggle(key: string) {
  picked.value = picked.value.includes(key) ? picked.value.filter((entry) => entry !== key) : [...picked.value, key]
}
</script>

<template>
  <div class="grid gap-2">
    <div v-for="section in sections" :key="section.group" class="grid gap-1">
      <p class="text-xs text-muted-foreground">{{ t(`categoryGroup.${section.group}`) }}</p>
      <div class="flex flex-wrap gap-1.5">
        <button
          v-for="category in section.items"
          :key="category.key"
          type="button"
          class="rounded-full border px-2.5 py-0.5 text-xs font-medium transition-colors focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          :class="
            picked.includes(category.key)
              ? 'border-transparent bg-primary text-primary-foreground'
              : 'border-border text-muted-foreground hover:bg-accent hover:text-accent-foreground'
          "
          :aria-pressed="picked.includes(category.key)"
          @click="toggle(category.key)"
        >
          {{ categoryLabel(category, t) }}
        </button>
      </div>
    </div>
  </div>
</template>
