<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Lock } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { categoryLabel } from '@/shared/lib/categories'

/**
 * «Системные · N» (Settings-1 · 4): the app's own categories. A lock, the name
 * as text, what it means and a badge; no fields and no actions, because core
 * refuses to change them. The empty fourth column keeps the rows level with the others.
 */
defineProps<{ items: Category[] }>()
const { t, te } = useI18n()
const meaning = (key: string) => (te(`categories.system.${key}`) ? t(`categories.system.${key}`) : '')
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)]" data-category-system>
    <div class="mt-0.5 mb-1 flex items-start gap-2 rounded-lg bg-muted px-3 py-2 text-sm text-muted-foreground">
      <Lock class="mt-0.5 size-4 shrink-0" aria-hidden="true" />
      <span class="min-w-0 flex-1">{{ t('categories.manage.systemHint') }}</span>
    </div>
    <div v-for="category in items" :key="category.key" class="flex min-h-13 items-center gap-2 border-b md:min-h-11.5 md:gap-3" data-category-row>
      <Lock class="size-4 shrink-0 text-muted-foreground md:ml-3" aria-hidden="true" />
      <span class="flex min-w-0 flex-1 flex-col md:flex-row md:items-center md:gap-3">
        <span class="truncate md:flex-1">{{ categoryLabel(category, t) }}</span>
        <span class="truncate text-xs text-muted-foreground md:flex-1">{{ meaning(category.key) }}</span>
      </span>
      <Badge variant="secondary" class="shrink-0">{{ t('categories.manage.systemBadge') }}</Badge>
    </div>
  </div>
</template>
