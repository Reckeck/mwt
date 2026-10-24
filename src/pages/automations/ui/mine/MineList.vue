<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { RouterLink, useRoute } from 'vue-router'

import { Switch } from '@/shared/ui/switch'

import { mineName, mineRoute, mineWhen } from '../../lib/mine-view'

/** «Мои» on a phone (Auto-1 · 1): the name, «Тип · дата» under it, the switch. */
const props = defineProps<{ items: MineItem[] }>()
const emit = defineEmits<{ toggle: [item: MineItem, active: boolean] }>()
const { t } = useI18n()
const route = useRoute()

const rows = computed(() =>
  props.items.map((item) => ({
    item,
    key: `${item.kind}:${item.id}`,
    name: mineName(item),
    sub: `${t(`automations.mine.kind.${item.kind}`)} · ${mineWhen(item, t)}`,
    to: mineRoute(item, route.query),
  })),
)
</script>

<template>
  <ul class="rounded-xl border border-border bg-card px-3 shadow-sm" data-mine-list>
    <li
      v-for="row in rows"
      :key="row.key"
      class="relative flex min-h-13 items-center gap-3 border-b border-border py-1.5 last:border-b-0"
      :data-mine-row="row.key"
    >
      <RouterLink :to="row.to" class="flex min-w-0 flex-1 flex-col after:absolute after:inset-0 focus-visible:outline-none">
        <span class="truncate font-medium">{{ row.name }}</span>
        <span class="truncate text-xs text-muted-foreground">{{ row.sub }}</span>
      </RouterLink>
      <Switch
        class="relative z-10"
        :model-value="row.item.active"
        :aria-label="t('automations.mine.toggle', { name: row.name })"
        @update:model-value="(value: boolean) => emit('toggle', row.item, value)"
      />
    </li>
  </ul>
</template>
