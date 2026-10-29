<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ChevronLeft, ListFilter, X } from 'lucide-vue-next'

import { useIsPhone } from '@/shared/lib/phone'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { Sheet, SheetContent, SheetTitle } from '@/shared/ui/sheet'

import type { FilterBarProps } from './filter-bar.types'

/**
 * A row of filters without labels (each field names itself to a screen
 * reader): `lead` (the search) and the most used ones (default slot), then
 * `More filters`, `actions` (export) and `trail` (the count) on the right.
 * `More filters` opens a panel under the row (`more`, `switches`): it edits a
 * draft the owner keeps; `Show N` applies it, `Reset` clears it. On a
 * phone only `lead` stays in the row; an icon opens the same panel as a screen
 * of its own, `‹ Filters`; the owner adds the row's fields to `more` there.
 */
const props = withDefaults(defineProps<FilterBarProps>(), {
  active: 0,
  moreActive: 0,
  folds: true,
  applyLabel: '',
  applyLabelFull: '',
})
const emit = defineEmits<{ reset: []; apply: []; clear: [] }>()
defineSlots<{
  default(): unknown
  lead?(): unknown
  more?(): unknown
  switches?(): unknown
  actions?(): unknown
  trail?(): unknown
  /** Under the row: a phone's chips of the set filters. */
  below?(): unknown
}>()
const open = defineModel<boolean>('open', { default: false })

const { t } = useI18n()
const phone = useIsPhone()

function apply() {
  emit('apply')
  open.value = false
}
</script>

<template>
  <search class="grid gap-3">
    <div class="flex flex-wrap items-center gap-3">
      <slot name="lead" />

      <Button
        v-if="phone && props.folds"
        variant="outline"
        size="icon"
        class="relative size-9 shrink-0"
        :aria-label="t('filter.open')"
        :aria-expanded="open"
        data-filter-more
        @click="open = true"
      >
        <ListFilter class="size-4" />
        <!-- A dot, not a count: the chips under the row name what is set. -->
        <span v-if="props.active" class="absolute top-1.5 right-1.5 size-2 rounded-full bg-destructive ring-2 ring-background" data-filter-dot />
        <span v-if="props.active" class="sr-only">{{ t('filter.activeCount', { count: props.active }) }}</span>
      </Button>
      <slot v-if="phone && props.folds" name="actions" />

      <template v-else-if="!phone">
        <slot />
        <Button v-if="$slots.more" variant="outline" class="shrink-0" :aria-expanded="open" data-filter-more @click="open = !open">
          <ListFilter class="size-4" />
          {{ t('filter.more') }}
          <Badge v-if="props.moreActive" variant="secondary" class="h-5 px-1.5 tabular-nums">{{ props.moreActive }}</Badge>
        </Button>
        <Button v-if="props.active > 0" variant="ghost" class="shrink-0" @click="emit('reset')">
          <X class="size-4" />
          {{ t('filter.reset', { count: props.active }) }}
        </Button>
        <slot name="actions" />
        <div class="ms-auto flex-1 basis-24 text-right"><slot name="trail" /></div>
      </template>
    </div>

    <slot name="below" />

    <!-- A computer: the folded filters open under the row, not over the list. -->
    <div v-if="!phone && open && $slots.more" class="grid gap-3 rounded-xl border border-border bg-card p-4 shadow-xs" data-filter-panel>
      <div class="grid grid-cols-1 gap-3 @2xl:grid-cols-2 @4xl:grid-cols-4">
        <slot name="more" />
      </div>
      <div class="flex flex-wrap items-center gap-3">
        <slot name="switches" />
        <span class="flex-1" />
        <Button variant="ghost" size="sm" class="text-muted-foreground" @click="emit('clear')">{{ t('period.reset') }}</Button>
        <Button size="sm" data-filter-apply @click="apply">{{ props.applyLabel }}</Button>
      </div>
    </div>

    <!-- A phone: a screen of its own; «‹» and the system `Back` close it unapplied. -->
    <Sheet v-if="phone && props.folds" v-model:open="open">
      <SheetContent side="right" class="gap-0 p-0" hide-close data-filter-panel>
        <div class="flex h-14 shrink-0 items-center gap-1 px-2">
          <Button variant="ghost" size="icon" class="size-10" :aria-label="t('actions.close')" @click="open = false">
            <ChevronLeft class="size-5" />
          </Button>
          <SheetTitle class="flex-1 text-xl font-semibold">{{ t('filter.open') }}</SheetTitle>
          <Button variant="link" class="px-2" @click="emit('clear')">{{ t('period.reset') }}</Button>
        </div>
        <div class="grid min-h-0 flex-1 content-start gap-4 overflow-y-auto overscroll-contain px-4 pb-4">
          <slot name="more" />
          <div class="grid gap-3"><slot name="switches" /></div>
        </div>
        <div class="shrink-0 px-4 pt-2 pb-4">
          <Button class="h-12 w-full text-base" data-filter-apply @click="apply">{{ props.applyLabelFull || props.applyLabel }}</Button>
        </div>
      </SheetContent>
    </Sheet>
  </search>
</template>
