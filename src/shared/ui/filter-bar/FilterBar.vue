<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { SlidersHorizontal, X } from 'lucide-vue-next'

import { useIsPhone } from '@/shared/lib/phone'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Drawer, DrawerContent, DrawerFooter, DrawerHeader, DrawerTitle } from '@/shared/ui/drawer'
import { Popover, PopoverContent, PopoverTrigger } from '@/shared/ui/popover'

import type { FilterBarProps } from './filter-bar.types'

/**
 * A row of filters: `lead` (the search, the period) and the ones used most
 * stay in view (default slot), the rest fold into «Ещё фильтры» (`more` slot)
 * with a count of what is set there, and «Сбросить» clears everything. Fields
 * wrap on a narrow window; each takes `FILTER_FIELD_CLASS`. On a phone only
 * `lead` stays in the row: the rest open from «Фильтры (N)» in a sheet.
 */
const props = withDefaults(defineProps<FilterBarProps>(), { active: 0, moreActive: 0 })
const emit = defineEmits<{ reset: [] }>()
defineSlots<{ default(): unknown; lead?(): unknown; more?(): unknown; actions?(): unknown }>()

const { t } = useI18n()
const phone = useIsPhone()
const sheetOpen = ref(false)
</script>

<template>
  <search class="flex flex-wrap items-end gap-3">
    <slot name="lead" />

    <template v-if="phone">
      <Button variant="outline" class="shrink-0" @click="sheetOpen = true">
        <SlidersHorizontal class="size-4" />
        {{ t('filter.open') }}
        <Badge v-if="props.active" variant="secondary" class="h-5 px-1.5 tabular-nums">{{ props.active }}</Badge>
      </Button>
      <Button
        v-if="props.active > 0"
        variant="ghost"
        size="icon"
        class="shrink-0"
        :title="t('filter.reset', { count: props.active })"
        :aria-label="t('filter.reset', { count: props.active })"
        @click="emit('reset')"
      >
        <X class="size-4" />
      </Button>
      <Drawer v-model:open="sheetOpen">
        <DrawerContent>
          <DrawerHeader class="pb-2"><DrawerTitle>{{ t('filter.open') }}</DrawerTitle></DrawerHeader>
          <div class="grid min-h-0 gap-4 overflow-y-auto overscroll-contain px-4 pb-2">
            <slot />
            <slot name="more" />
          </div>
          <DrawerFooter class="flex-row justify-end">
            <Button v-if="props.active > 0" variant="ghost" @click="emit('reset')">
              {{ t('filter.reset', { count: props.active }) }}
            </Button>
            <Button @click="sheetOpen = false">{{ t('ui.done') }}</Button>
          </DrawerFooter>
        </DrawerContent>
      </Drawer>
    </template>

    <template v-else>
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
    </template>

    <div v-if="$slots.actions" class="ml-auto flex shrink-0 items-end gap-2">
      <slot name="actions" />
    </div>
  </search>
</template>
