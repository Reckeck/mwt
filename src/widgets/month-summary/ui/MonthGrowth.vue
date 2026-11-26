<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { useCategories } from '@/shared/api/queries'
import { categoryKeyLabel } from '@/shared/lib/categories'
import { EntityLink } from '@/shared/ui/entity-link'

/** Up to three categories that grew most against the previous month to the same date. */
defineProps<{ growth: CategoryGrowth[]; currency: string }>()
const { t } = useI18n()
const categories = useCategories()
</script>

<template>
  <div v-if="growth.length" class="grid gap-1 text-xs">
    <p class="text-muted-foreground">{{ t('monthSummary.grew') }}</p>
    <ul class="grid gap-1">
      <li v-for="row in growth" :key="row.category" class="flex items-center justify-between gap-2 tabular-nums">
        <EntityLink
          :target="{ kind: 'category', id: row.category }"
          :label="categoryKeyLabel(row.category, categories.data.value, t)"
          class="min-w-0 truncate"
        />
        <Money
          class="text-destructive"
          :amount="Number(row.value) - Number(row.base)"
          :currency="currency"
          :decimals="0"
          signed
          :convert="false"
        />
      </li>
    </ul>
  </div>
</template>
