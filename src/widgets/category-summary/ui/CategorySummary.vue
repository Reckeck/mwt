<script setup lang="ts">
import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Lock, MoreHorizontal, Plus } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { useMonthCategoryTotals } from '@/entities/report'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'
import { useCategories } from '@/shared/api/queries'
import { categoryLabel } from '@/shared/lib/categories'
import { formatMonthKey } from '@/shared/lib/format'
import { settingsQuery } from '@/shared/lib/settings-link'

/**
 * «Категории» on Учёт › Общее (Settings-1 · 3): a short sample — one live
 * category of each main group — and the service ones in a line. Editing lives
 * on the «Категории» tab; every control here leads there.
 */
const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const categories = useCategories()
const { month, totals, currency } = useMonthCategoryTotals()
const monthName = computed(() => formatMonthKey(month, { month: 'short' }))

const SAMPLE_GROUPS: CategoryGroup[] = ['daily', 'fixed', 'income']
const live = computed(() => (categories.data.value ?? []).filter((category) => !category.archived))
const sample = computed(() =>
  SAMPLE_GROUPS.flatMap((group) => live.value.filter((category) => category.group === group).slice(0, 1)),
)
const system = computed(() => live.value.filter((category) => category.group === 'service'))
const systemLine = computed(() => {
  const names = system.value.slice(0, 2).map((category) => categoryLabel(category, t)).join(', ')
  const rest = system.value.length - 2
  return rest > 0 ? t('categorySummary.systemMore', { names, n: rest }) : t('categorySummary.system', { names })
})

const openCategories = () => void router.replace({ query: settingsQuery(route.query, 'accounting', 'categories') })
</script>

<template>
  <SettingsSection :title="t('settingsShell.section.categories')">
    <div class="flex justify-end">
      <Button variant="outline" size="sm" class="touch-hit" data-category-add @click="openCategories">
        <Plus class="size-4" />{{ t('categorySummary.add') }}
      </Button>
    </div>
    <div class="-mt-2 flex min-w-0 flex-col">
      <Button
        v-for="category in sample"
        :key="category.key"
        variant="ghost"
        class="h-auto min-h-10 w-full min-w-0 justify-start gap-2 rounded-none border-b px-0 font-normal"
        @click="openCategories"
      >
        <span class="min-w-0 flex-1 truncate text-left">{{ categoryLabel(category, t) }}</span>
        <span v-if="category.group === 'income'" class="shrink-0 text-xs text-muted-foreground">{{ t('categorySummary.income') }}</span>
        <span v-else-if="totals.get(category.key)" class="flex shrink-0 gap-1 text-xs text-muted-foreground">
          <Money :amount="totals.get(category.key) ?? '0'" :currency="currency" :decimals="0" />{{ t('categories.inMonth', { month: monthName }) }}
        </span>
        <span v-else class="shrink-0 text-xs text-muted-foreground">{{ t(`categoryGroup.${category.group}`) }}</span>
        <MoreHorizontal class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      </Button>
      <div v-if="system.length" class="flex min-h-10 items-center gap-2">
        <Lock class="size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
        <span class="min-w-0 flex-1 truncate text-sm">{{ systemLine }}</span>
        <Button variant="link" size="sm" class="h-auto touch-hit shrink-0 p-0 text-xs" data-category-all @click="openCategories">
          {{ t('categorySummary.all') }}
        </Button>
      </div>
    </div>
  </SettingsSection>
</template>
