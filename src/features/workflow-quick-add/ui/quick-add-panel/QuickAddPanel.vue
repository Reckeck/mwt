<script setup lang="ts">
import { computed, nextTick, onMounted, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { onClickOutside } from '@vueuse/core'
import { Sparkles } from 'lucide-vue-next'

import { useNodeCatalog } from '@/entities/workflow'
import { Input } from '@/shared/ui/input'
import { cn } from '@/shared/lib/utils'
import { GROUP_COLOR, NODE_GROUPS, localized } from '@/shared/lib/workflow'
import { Button } from '@/shared/ui/button'

import { type QuickAddChoice, fitFor, matchScore, quickExpression } from '../../model/quick-add'
import type { QuickAddPanelProps } from './quick-add-panel.types'

/**
 * Ctrl+Space: a search box at the cursor. It offers only the nodes that fit
 * where it was opened — after a port, those that take its value; on a wire,
 * those that can sit in it — and turns `10%`, `> 5000`, `= 100` into nodes.
 */
const props = defineProps<QuickAddPanelProps>()
const emit = defineEmits<{ pick: [choice: QuickAddChoice]; close: [] }>()

const { t, locale } = useI18n()
const { catalog, def } = useNodeCatalog()
const query = ref('')
const active = ref(0)
const field = ref<InstanceType<typeof Input> | null>(null)
const list = ref<HTMLElement | null>(null)
const panel = ref<HTMLElement | null>(null)

// A click anywhere else closes the panel, like Escape.
onClickOutside(panel, () => emit('close'))

interface Row {
  choice: QuickAddChoice
  def: NodeDef
}

const rows = computed<Row[]>(() => {
  const out: Row[] = []
  const expression = quickExpression(query.value)
  const expressionDef = expression ? def(expression.kind) : undefined
  if (expression && expressionDef) {
    // An expression that does not fit here is still added, just not wired.
    const fit = fitFor(expressionDef, props.context)
    const choice = fit ? { ...expression, port: fit.port ?? expression.port, outPort: fit.outPort ?? expression.outPort } : { ...expression, port: null }
    out.push({ choice, def: expressionDef })
  }
  const scored = catalog.value
    .filter((entry) => !props.simple || !entry.advanced || query.value.trim())
    .map((entry) => ({ entry, score: matchScore(entry, query.value), fit: fitFor(entry, props.context) }))
    .filter((item) => item.score > 0 && item.fit)
    .sort(
      (left, right) =>
        right.score - left.score || NODE_GROUPS.indexOf(left.entry.group) - NODE_GROUPS.indexOf(right.entry.group),
    )
  for (const item of scored) {
    if (!item.fit) continue
    out.push({ def: item.entry, choice: { kind: item.entry.kind, params: {}, port: item.fit.port, outPort: item.fit.outPort, hint: '' } })
  }
  return out.slice(0, 40)
})

watch(query, () => {
  active.value = 0
})

function pick(row: Row | undefined) {
  if (row) emit('pick', row.choice)
}

function onKey(event: KeyboardEvent) {
  if (event.key === 'ArrowDown') {
    active.value = Math.min(active.value + 1, rows.value.length - 1)
  } else if (event.key === 'ArrowUp') {
    active.value = Math.max(active.value - 1, 0)
  } else if (event.key === 'Enter') {
    pick(rows.value[active.value])
  } else if (event.key === 'Escape') {
    emit('close')
  } else {
    return
  }
  event.preventDefault()
  void nextTick(() => list.value?.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' }))
}

const contextText = computed(() => {
  if (props.context.mode === 'port') return t(props.context.side === 'out' ? 'workflow.quick.afterPort' : 'workflow.quick.beforePort')
  if (props.context.mode === 'edge') return t('workflow.quick.intoWire')
  return t('workflow.quick.free')
})

onMounted(() => {
  void nextTick(() => (field.value?.$el as HTMLInputElement | undefined)?.focus())
})
</script>

<template>
  <div
    ref="panel"
    class="fixed z-50 grid w-80 gap-2 rounded-lg border border-border bg-popover p-2 text-popover-foreground shadow-lg"
    :style="{ left: `${props.left}px`, top: `${props.top}px` }"
    data-tour="workflow-quick-add"
    role="dialog"
    :aria-label="contextText"
    @keydown="onKey"
  >
    <p class="px-1 text-xs text-muted-foreground">{{ contextText }}</p>
    <Input ref="field" v-model="query" :placeholder="t('workflow.quick.placeholder')" :aria-label="t('workflow.quick.placeholder')" />
    <ul ref="list" class="max-h-72 overflow-y-auto">
      <li v-for="(row, index) in rows" :key="`${row.choice.kind}-${row.choice.hint}`">
        <Button
          variant="ghost"
          size="block"
          :data-active="index === active"
          :aria-current="index === active ? 'true' : undefined"
          :class="cn('flex w-full items-start gap-2 rounded-md px-2 py-1.5 text-left text-sm', index === active ? 'bg-accent' : 'hover:bg-muted')"
          @mouseenter="active = index"
          @click="pick(row)"
        >
          <span :class="cn('mt-1.5 size-2 shrink-0 rounded-full', GROUP_COLOR[row.def.group])" />
          <span class="min-w-0 flex-1">
            <span class="flex items-center gap-1 font-medium">
              <Sparkles v-if="row.choice.hint" class="size-3.5 text-primary" />
              {{ row.choice.hint ? `${localized(row.def.title, locale)}: ${row.choice.hint}` : localized(row.def.title, locale) }}
            </span>
            <span class="block truncate text-xs text-muted-foreground">{{ localized(row.def.description, locale) }}</span>
          </span>
        </Button>
      </li>
      <li v-if="!rows.length" class="px-2 py-3 text-sm text-muted-foreground">{{ t('workflow.quick.none') }}</li>
    </ul>
    <p class="px-1 text-xs text-muted-foreground">{{ t('workflow.quick.tips') }}</p>
  </div>
</template>
