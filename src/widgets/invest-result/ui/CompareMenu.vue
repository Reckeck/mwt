<script setup lang="ts">
import { useI18n } from 'vue-i18n'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Checkbox } from '@/shared/ui/checkbox'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

import { type CompareLines, MAX_COMPARED } from '../model/use-compare-lines'

/**
 * «Сравнить»: the variants of «Что выгоднее» and the saved scenarios with a
 * tick each, up to three; the picked ones become lines on the chart (Money 4 · 22).
 */
const props = defineProps<{ compare: CompareLines }>()

const { t } = useI18n()
const full = () => props.compare.picked.value.length >= MAX_COMPARED
</script>

<template>
  <Popover>
    <PopoverTrigger as-child>
      <Button variant="outline" data-tour="calc-overlay" data-calc-compare>
        {{ t('invest.compareWith.button') }}
        <Badge v-if="props.compare.picked.value.length" variant="secondary">{{ props.compare.picked.value.length }}</Badge>
      </Button>
    </PopoverTrigger>
    <PopoverContent overlay="calc-compare" align="end" class="grid w-[min(18rem,calc(100vw-2rem))] gap-1 p-2" data-calc-compare-menu>
      <p class="px-2 py-1 text-xs font-medium text-muted-foreground">{{ t('invest.compareWith.title', { max: MAX_COMPARED }) }}</p>
      <label
        v-for="option in props.compare.options.value"
        :key="option.key"
        :for="`calc-compare-${option.key}`"
        class="touch-hit flex cursor-pointer items-center gap-2 rounded-md px-2 py-1.5 text-sm hover:bg-accent has-disabled:cursor-default has-disabled:opacity-50"
      >
        <Checkbox
          :id="`calc-compare-${option.key}`"
          :model-value="props.compare.picked.value.includes(option.key)"
          :disabled="full() && !props.compare.picked.value.includes(option.key)"
          @update:model-value="props.compare.toggle(option.key)"
        />
        <span class="min-w-0 flex-1 truncate">{{ option.name }}</span>
        <span class="text-xs text-muted-foreground">{{ t(`invest.compareWith.tag.${option.tag}`) }}</span>
      </label>
      <p v-if="!props.compare.options.value.length" class="px-2 py-1 text-sm text-muted-foreground">{{ t('invest.compareWith.empty') }}</p>
    </PopoverContent>
  </Popover>
</template>
