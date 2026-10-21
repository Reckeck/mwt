<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { Trash2 } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { formatDate } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'

import type { ValuationListProps } from './valuation-list.types'

/** The dated valuations of one asset, newest first, each removable. */
const props = defineProps<ValuationListProps>()
const emit = defineEmits<{ remove: [id: string] }>()
const { t } = useI18n()
</script>

<template>
  <div class="grid gap-1">
    <p class="text-xs font-medium text-muted-foreground">{{ t('valuation.history') }}</p>
    <ul class="grid divide-y rounded-md border">
      <li v-if="props.base" class="flex min-h-10 items-center justify-between gap-3 px-3 py-1.5 text-sm">
        <span class="text-muted-foreground">
          {{ formatDate(props.base.date) }} · {{ t(`valuation.base.${props.base.kind}`) }}
        </span>
        <Money :amount="props.base.price" :currency="props.base.currency" :decimals="0" />
      </li>
      <li
        v-for="row in [...props.valuations].reverse()"
        :key="row.id"
        class="flex min-h-10 items-center justify-between gap-3 px-3 py-1 text-sm"
      >
        <span>{{ formatDate(row.date) }}</span>
        <span class="flex items-center gap-1">
          <Money :amount="row.unit_price" :currency="row.currency" :decimals="0" />
          <Button
            variant="ghost"
            size="icon"
            :aria-label="t('valuation.delete')"
            :disabled="props.pending"
            @click="emit('remove', row.id)"
          >
            <Trash2 class="size-4" />
          </Button>
        </span>
      </li>
    </ul>
  </div>
</template>
