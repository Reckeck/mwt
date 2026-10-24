<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, Search } from 'lucide-vue-next'

import { useMonthCategoryTotals } from '@/entities/report'
import { useCategories } from '@/shared/api/queries'
import { categoryLabel } from '@/shared/lib/categories'
import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'

import { SECTIONS } from './category-manager.constants'
import { useCategoryActions } from './model/use-category-actions'
import CategoryAddDialog from './ui/CategoryAddDialog.vue'
import CategoryEditDialog from './ui/CategoryEditDialog.vue'
import CategoryRow from './ui/CategoryRow.vue'
import SystemCategories from './ui/SystemCategories.vue'

/**
 * Учёт › «Категории» (Settings-1 · 4): search and «+ Категория», then the
 * categories by block (expenses, income, savings) and the app's own below,
 * locked. The change itself is the backend's: system categories are refused there.
 */
const { t } = useI18n()
const categories = useCategories()
const actions = useCategoryActions()
const { month, totals, currency } = useMonthCategoryTotals()

const query = ref('')
const adding = ref(false)
const editingKey = ref<string | null>(null)
const editing = computed(() => (categories.data.value ?? []).find((category) => category.key === editingKey.value) ?? null)
const editorOpen = computed({
  get: () => editing.value !== null,
  set: (open) => {
    if (!open) editingKey.value = null
  },
})

const matches = (category: Category) => categoryLabel(category, t).toLowerCase().includes(query.value.trim().toLowerCase())
const all = computed(() => (categories.data.value ?? []).filter(matches))
const blocks = computed(() =>
  SECTIONS.map((section) => ({
    ...section,
    items: section.groups.flatMap((group) => all.value.filter((category) => category.group === group)),
  })).filter((block) => block.items.length),
)
const system = computed(() => all.value.filter((category) => category.group === 'service'))
/** The month is named once, beside the first sum on the screen. */
const firstWithSum = computed(() => blocks.value.flatMap((block) => block.items).find((category) => totals.value.has(category.key))?.key)
</script>

<template>
  <div class="flex flex-col" data-category-manager>
    <div class="flex items-center gap-2">
      <div class="relative min-w-0 flex-1 md:w-65 md:flex-none">
        <Search class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground" aria-hidden="true" />
        <Input v-model="query" type="search" class="touch-hit pl-8" :placeholder="t('categories.manage.search')" :aria-label="t('categories.manage.search')" />
      </div>
      <div class="md:flex-1" />
      <Button variant="outline" size="sm" class="touch-hit shrink-0" data-category-add @click="adding = true">
        <Plus class="size-4" />{{ t('categorySummary.add') }}
      </Button>
    </div>

    <p v-if="!all.length" class="py-6 text-center text-sm text-muted-foreground">{{ t('categories.manage.noMatch') }}</p>

    <template v-for="block in blocks" :key="block.id">
      <p class="pt-2.5 pb-0.5 text-xs font-medium text-muted-foreground">{{ t(block.title) }}</p>
      <CategoryRow
        v-for="category in block.items"
        :key="category.key"
        :category="category"
        :actions="actions"
        :total="totals.get(category.key)"
        :currency="currency"
        :month="month"
        :show-month="category.key === firstWithSum"
        @edit="editingKey = category.key"
      />
    </template>

    <template v-if="system.length">
      <p class="pt-2.5 pb-0.5 text-xs font-medium text-muted-foreground">{{ t('categories.manage.systemTitle', { n: system.length }) }}</p>
      <SystemCategories :items="system" />
    </template>

    <CategoryAddDialog v-model:open="adding" :pending="actions.create.isPending.value" @create="(input) => actions.create.mutate(input, { onSuccess: () => (adding = false) })" />
    <CategoryEditDialog v-model:open="editorOpen" :category="editing" :actions="actions" />
  </div>
</template>
