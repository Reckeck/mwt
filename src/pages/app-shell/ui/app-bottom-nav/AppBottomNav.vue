<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { cn } from '@/shared/lib/utils'

import { BOTTOM_FAB_CLASS } from '../../lib/bottom-fab'
import { NAV_SECTIONS } from '../../lib/nav-sections'

/**
 * The phone's bar (Shell · phone bottom bar): the five sections, icon over label with a pill on the
 * active one, and a round «+» that floats above the bar's right edge and opens quick add.
 */
const emit = defineEmits<{ quickAdd: [] }>()

const { t } = useI18n()
const route = useRoute()

/** One row model per section: label and active state computed once, not per call in the template. */
const tabs = computed(() =>
  NAV_SECTIONS.map((section) => ({
    section,
    label: t(section.key),
    active: typeof route.name === 'string' && section.routes.includes(route.name),
  })),
)

/** Five equal columns: `basis-0` and no padding of its own. */
const ITEM =
  'flex min-h-12 min-w-0 shrink grow basis-0 flex-col items-center justify-center gap-0.5 rounded-md px-0.5 text-[0.6875rem] font-medium transition-colors ' +
  'focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring'
const PILL = 'flex h-7 w-12 items-center justify-center rounded-full transition-colors'
</script>

<template>
  <nav
    class="relative flex shrink-0 items-stretch gap-1 border-t border-border bg-card px-2 pt-1 pb-[calc(0.25rem+var(--safe-bottom))]"
    :aria-label="t('nav.bottom')"
    data-tour="bottom-nav"
  >
    <RouterLink
      v-for="tab in tabs"
      :key="tab.section.id"
      :to="{ name: tab.section.root }"
      :class="cn(ITEM, tab.active ? 'text-primary' : 'text-muted-foreground')"
      :aria-current="tab.active ? 'page' : undefined"
    >
      <span :class="cn(PILL, tab.active && 'bg-primary/10')">
        <component :is="tab.section.icon" class="size-5 shrink-0" />
      </span>
      <span class="max-w-full truncate">{{ tab.label }}</span>
    </RouterLink>
    <Button
      size="icon"
      :class="BOTTOM_FAB_CLASS"
      :aria-label="t('quickAdd.title')"
      data-bottom-fab
      data-tour="quick-add"
      @click="emit('quickAdd')"
    >
      <Plus class="size-6" />
    </Button>
  </nav>
</template>
