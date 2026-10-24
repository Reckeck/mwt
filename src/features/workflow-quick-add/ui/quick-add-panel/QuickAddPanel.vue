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
onClickOutside(panel, () => {
  if (!props.docked) emit('close')
})

interface Row {
  choice: QuickAddChoice
  def: NodeDef
}

/** An expression is the first row; the nodes that fit follow it, as with no search. */
const expression = computed(() => quickExpression(query.value))
const search = computed(() => (expression.value ? '' : query.value))

const rows = computed<Row[]>(() => {
  const out: Row[] = []
  const typed = expression.value
  const expressionDef = typed ? def(typed.kind) : undefined
  if (typed && expressionDef) {
    // An expression that does not fit here is still added, just not wired.
    const fit = fitFor(expressionDef, props.context)
    const choice = fit ? { ...typed, port: fit.port ?? typed.port, outPort: fit.outPort ?? typed.outPort } : { ...typed, port: null }
    out.push({ choice, def: expressionDef })
  }
  const scored = catalog.value
    .filter((entry) => !props.simple || !entry.advanced || search.value.trim())
    .map((entry) => ({ entry, score: matchScore(entry, search.value), fit: fitFor(entry, props.context) }))
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

/** Groups in the order an insert wants them (Auto-2 · 14); a search is one list, best first. */
const GROUP_ORDER: NodeGroup[] = ['action', 'logic', 'data', ...NODE_GROUPS.filter((group) => !['action', 'logic', 'data'].includes(group))]
const sections = computed(() => {
  const indexed = rows.value.map((row, index) => ({ row, index }))
  if (search.value.trim()) return [{ group: null as NodeGroup | null, items: indexed }]
  const found = indexed.filter((item) => item.row.choice.hint)
  const grouped = GROUP_ORDER.map((group) => ({
    group: group as NodeGroup | null,
    items: indexed.filter((item) => !item.row.choice.hint && item.row.def.group === group),
  }))
  return [{ group: null, items: found }, ...grouped].filter((section) => section.items.length)
})
const flat = computed(() => sections.value.flatMap((section) => section.items.map((item) => item.row)))

watch(query, () => {
  active.value = 0
})

function pick(row: Row | undefined) {
  if (row) emit('pick', row.choice)
}

function onKey(event: KeyboardEvent) {
  if (event.key === 'ArrowDown') {
    active.value = Math.min(active.value + 1, flat.value.length - 1)
  } else if (event.key === 'ArrowUp') {
    active.value = Math.max(active.value - 1, 0)
  } else if (event.key === 'Enter') {
    pick(flat.value[active.value])
  } else if (event.key === 'Escape') {
    emit('close')
  } else {
    return
  }
  event.preventDefault()
  void nextTick(() => list.value?.querySelector('[data-active="true"]')?.scrollIntoView({ block: 'nearest' }))
}

const text = (value: LocalizedText | string) => (typeof value === 'string' ? value : localized(value, locale.value))
/** «Вставить в связь «Добавлена операция → сумма»: …» — where the new node will stand. */
const contextText = computed(() => {
  const ctx = props.context
  if (ctx.mode === 'free') return t('workflow.quick.free')
  const wire = ctx.source ? `${text(ctx.source.node)} → ${text(ctx.source.port).toLowerCase()}` : ''
  if (ctx.mode === 'edge') return wire ? t('workflow.quick.intoWireNamed', { wire }) : t('workflow.quick.intoWire')
  if (ctx.side === 'out' && wire) return t('workflow.quick.afterPortNamed', { wire })
  return t(ctx.side === 'out' ? 'workflow.quick.afterPort' : 'workflow.quick.beforePort')
})

onMounted(() => {
  void nextTick(() => (field.value?.$el as HTMLInputElement | undefined)?.focus())
})
// check-ui-ignore fixed-width: floats at the cursor on a desktop only; a phone docks it in a sheet
</script>

<template>
  <div
    ref="panel"
    :class="
      props.docked
        ? 'grid min-h-0 gap-2 p-3'
        : 'fixed z-overlay grid w-80 gap-2 rounded-lg border border-border bg-popover p-2 text-popover-foreground shadow-lg'
    "
    :style="props.docked ? undefined : { left: `${props.left}px`, top: `${props.top}px` }"
    data-tour="workflow-quick-add"
    role="dialog"
    :aria-label="contextText"
    @keydown="onKey"
  >
    <Input ref="field" v-model="query" :placeholder="t('workflow.quick.placeholder')" :aria-label="t('workflow.quick.placeholder')" />
    <p v-if="!props.docked" class="px-1 text-xs text-muted-foreground" data-quick-context>{{ contextText }}</p>
    <ul ref="list" :class="props.docked ? 'max-h-[50dvh] overflow-y-auto' : 'max-h-72 overflow-y-auto'">
      <template v-for="section in sections" :key="section.group ?? 'found'">
        <li v-if="section.group" class="px-2 pt-2 pb-1 text-xs text-muted-foreground">{{ t(`workflow.group.${section.group}`) }}</li>
        <li v-for="{ row, index } in section.items" :key="`${row.choice.kind}-${row.choice.hint}`">
          <Button
            variant="ghost"
            size="block"
            :data-active="index === active"
            :aria-current="index === active ? 'true' : undefined"
            :title="localized(row.def.description, locale)"
            :class="
              cn(
                'flex w-full items-center gap-2 rounded-md px-2 text-left text-sm max-md:min-h-11',
                props.docked ? 'py-2.5' : 'py-1.5',
                index === active ? 'bg-accent' : 'hover:bg-muted',
              )
            "
            @mouseenter="active = index"
            @click="pick(row)"
          >
            <span :class="cn('size-2 shrink-0 rounded-full', GROUP_COLOR[row.def.group])" />
            <Sparkles v-if="row.choice.hint" class="size-3.5 shrink-0 text-primary" />
            <span class="min-w-0 flex-1 truncate">
              {{ row.choice.hint ? `${localized(row.def.title, locale)}: ${row.choice.hint}` : localized(row.def.title, locale) }}
            </span>
          </Button>
        </li>
      </template>
      <li v-if="!rows.length" class="px-2 py-3 text-sm text-muted-foreground">{{ t('workflow.quick.none') }}</li>
    </ul>
    <p v-if="!props.docked" class="px-1 text-xs text-muted-foreground">{{ t('workflow.quick.tips') }}</p>
  </div>
</template>
