<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { SlidersHorizontal, X } from 'lucide-vue-next'

import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

import type { FilterBarProps } from './filter-bar.types'

/**
 * A row of filters: the ones used most stay in view (default slot), the rest
 * fold into «Ещё фильтры» (`more` slot) with a count of what is set there,
 * and «Сбросить» clears everything. Fields wrap on a narrow window; each
 * field takes `class="min-w-36 flex-1 @3xl:flex-none"` from `FILTER_FIELD_CLASS`.
 */
const props = withDefaults(defineProps<FilterBarProps>(), { active: 0, moreActive: 0 })
const emit = defineEmits<{ reset: [] }>()
defineSlots<{ default(): unknown; more?(): unknown; actions?(): unknown }>()

const { t } = useI18n()
</script>

<template>
  <search class="flex flex-wrap items-end gap-3">
    <slot />
    <Popover v-if="$slots.more">
      <PopoverTrigger as-child>
        <Button variant="outline" class="shrink-0">
          <SlidersHorizontal class="size-4" />
          {{ t('filter.more') }}
          <Badge v-if="props.moreActive" variant="secondary" class="h-5 px-1.5 tabular-nums">{{ props.moreActive }}</Badge>
        </Button>
      </PopoverTrigger>
      <PopoverContent align="start" class="grid w-80 max-w-[calc(100vw-2rem)] gap-4">
        <slot name="more" />
      </PopoverContent>
    </Popover>
    <Button v-if="props.active > 0" variant="ghost" class="shrink-0" @click="emit('reset')">
      <X class="size-4" />
      {{ t('filter.reset', { count: props.active }) }}
    </Button>
    <div v-if="$slots.actions" class="ml-auto flex shrink-0 items-end gap-2">
      <slot name="actions" />
    </div>
  </search>
</template>
