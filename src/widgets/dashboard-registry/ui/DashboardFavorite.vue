<script setup lang="ts">
import { computed } from 'vue'
import { RouterLink, useRoute } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { Money } from '@/entities/currency'
import { usePins } from '@/shared/api/pins'
import { entityLocation } from '@/shared/lib/entity-link'
import { Card } from '@/shared/ui/card'
import { SectionState } from '@/shared/ui/section-state'

/** `Favourite wallet` (Home 4): the first wallet pinned to the sidebar, its balance and a way in. */
const { t } = useI18n()
const route = useRoute()
const pins = usePins()
const view = computed(() =>
  (pins.query.data.value ?? []).find((item) => item.pin.type === 'entity' && item.pin.target.kind === 'store' && !item.missing),
)
const to = computed(() => (view.value && view.value.pin.type === 'entity' ? entityLocation(view.value.pin.target, route.query) : null))
</script>

<template>
  <Card size="sm">
    <SectionState :loading="pins.query.isPending.value" :empty="!view" :empty-title="t('dashboard.favorite.none')" :empty-description="t('dashboard.favorite.hint')" compact>
      <div v-if="view && to" class="grid gap-1" data-tour="dash-favorite">
        <p class="truncate text-xs text-muted-foreground">{{ t('dashboard.widget.favorite') }}</p>
        <RouterLink :to="to" class="truncate rounded-sm font-medium hover:underline focus-visible:outline-2 focus-visible:outline-ring">{{ view.title }}</RouterLink>
        <p v-if="view.value !== null" class="text-2xl font-semibold tabular-nums">
          <Money :amount="view.value" :currency="view.currency ?? undefined" />
        </p>
      </div>
    </SectionState>
  </Card>
</template>
