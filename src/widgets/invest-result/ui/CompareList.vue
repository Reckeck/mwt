<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ChevronRight } from 'lucide-vue-next'

import { Money } from '@/entities/currency'
import { Button } from '@/shared/ui/button'

import type { CompareLine } from '../model/use-compare-lines'

/**
 * «Сравнение» on a phone: where each run ends, a row each. A thin line on a
 * small chart is no target for a finger, so the row opens the run instead.
 */
const props = defineProps<{ current: string; lines: CompareLine[]; currency: string }>()
const emit = defineEmits<{ open: [target: CompareLine['open']] }>()

const { t } = useI18n()
</script>

<template>
  <section class="grid gap-2 @md:hidden" data-calc-compare-list>
    <h3 class="text-sm font-medium">{{ t('invest.compareWith.listTitle') }}</h3>
    <ul class="grid divide-y rounded-md border">
      <li class="flex items-center justify-between gap-3 px-3 py-2.5 text-sm">
        <span class="font-medium">{{ t('invest.compareWith.current') }}</span>
        <Money :amount="props.current" :currency="props.currency" :decimals="0" class="font-medium" />
      </li>
      <li v-for="line in props.lines" :key="line.key">
        <Button
          variant="ghost"
          class="h-auto w-full justify-between gap-3 rounded-none px-3 py-2.5 text-left font-normal"
          @click="emit('open', line.open)"
        >
          <span class="min-w-0 truncate">{{ line.name }}</span>
          <span class="flex items-center gap-1">
            <Money :amount="line.final" :currency="props.currency" :decimals="0" />
            <ChevronRight class="size-4 text-muted-foreground" />
          </span>
        </Button>
      </li>
    </ul>
  </section>
</template>
