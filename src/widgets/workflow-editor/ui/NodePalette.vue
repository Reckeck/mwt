<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Search } from 'lucide-vue-next'

import { useNodeCatalog } from '@/entities/workflow'
import { Input } from '@/shared/ui/input'
import { cn } from '@/shared/lib/utils'
import { GROUP_COLOR, NODE_GROUPS, localized } from '@/shared/lib/workflow'

/** Every node by group: drag one onto the canvas or click to add it. */
const props = defineProps<{ simple: boolean }>()
const emit = defineEmits<{ add: [kind: string] }>()

const { t, locale } = useI18n()
const { catalog } = useNodeCatalog()
const query = ref('')

const groups = computed(() => {
  const needle = query.value.trim().toLowerCase()
  return NODE_GROUPS.map((group) => ({
    group,
    nodes: catalog.value.filter(
      (def) =>
        def.group === group &&
        (!props.simple || !def.advanced) &&
        (!needle ||
          `${def.title.ru} ${def.title.en} ${def.keywords.join(' ')}`.toLowerCase().includes(needle)),
    ),
  })).filter((entry) => entry.nodes.length)
})

function onDragStart(event: DragEvent, kind: string) {
  event.dataTransfer?.setData('application/x-workflow-node', kind)
  if (event.dataTransfer) event.dataTransfer.effectAllowed = 'move'
}
</script>

<template>
  <aside class="flex min-h-0 flex-col gap-2" data-tour="workflow-palette">
    <div class="relative">
      <Search class="pointer-events-none absolute left-2.5 top-2.5 size-4 text-muted-foreground" />
      <Input v-model="query" class="pl-8" :placeholder="t('workflow.palette.search')" :aria-label="t('workflow.palette.search')" />
    </div>
    <div class="min-h-0 flex-1 overflow-y-auto pr-1">
      <section v-for="entry in groups" :key="entry.group" class="mb-3">
        <h3 class="mb-1 flex items-center gap-1.5 text-xs font-medium uppercase text-muted-foreground">
          <span :class="cn('size-2 rounded-full', GROUP_COLOR[entry.group])" />
          {{ t(`workflow.group.${entry.group}`) }}
        </h3>
        <button
          v-for="def in entry.nodes"
          :key="def.kind"
          type="button"
          draggable="true"
          class="block w-full cursor-grab rounded-md px-2 py-1.5 text-left hover:bg-muted focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
          :title="localized(def.description, locale)"
          @dragstart="onDragStart($event, def.kind)"
          @click="emit('add', def.kind)"
        >
          <span class="block text-sm">{{ localized(def.title, locale) }}</span>
          <span class="block truncate text-xs text-muted-foreground">{{ localized(def.description, locale) }}</span>
        </button>
      </section>
      <p v-if="!groups.length" class="px-2 text-sm text-muted-foreground">{{ t('workflow.palette.none') }}</p>
    </div>
    <p class="text-xs text-muted-foreground">{{ t('workflow.palette.hint') }}</p>
  </aside>
</template>
