<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { ChevronRight, Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { pluralForm } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'
import { GROUP_COLOR, localized } from '@/shared/lib/workflow'

import { nodeProblem } from '../model/node-problem'
import { schemeSteps } from '../model/scheme-steps'
import type { EditorGraph } from '../model/use-editor-graph'
import type { useSchemeWords } from '../model/use-scheme-words'

/**
 * The phone's scheme (Auto-2 · 13): the steps as a list in run order, the
 * data nodes folded into one line over it. A tap opens the step's fields in
 * the sheet; `Canvas` shows the nodes and wires instead. The last emulation
 * numbers the steps and puts its card under them (the default slot).
 */
const props = defineProps<{
  editor: EditorGraph
  scheme: ReturnType<typeof useSchemeWords>
  issuesOf: (id: string) => GraphIssue[]
  order?: Map<string, number>
}>()
const canvas = defineModel<boolean>('canvas', { required: true })
const emit = defineEmits<{ add: [] }>()

const { t, locale } = useI18n()
const { editor } = props

const split = computed(() => schemeSteps(editor.nodes.value, editor.edges.value, editor.def))
const selected = computed(() => editor.flow.getSelectedNodes.value[0]?.id ?? null)
const summary = (id: string) => {
  const node = editor.nodeById(id)
  return props.scheme.summaryOf(id, node?.data?.kind ?? '', node?.data?.params ?? {})
}
const rows = computed(() =>
  split.value.steps.map((id) => {
    const node = editor.nodeById(id)
    const nodeDef = editor.def(node?.data?.kind ?? '')
    return {
      id,
      title: node?.data?.label || (nodeDef ? localized(nodeDef.title, locale.value) : (node?.data?.kind ?? '')),
      value: summary(id),
      problem: nodeProblem(props.issuesOf(id), nodeDef, locale.value, t),
      color: nodeDef ? GROUP_COLOR[nodeDef.group] : 'bg-muted-foreground',
      disabled: node?.data?.disabled ?? false,
      order: props.order?.get(id) ?? null,
    }
  }),
)
const meta = computed(() => {
  const count = rows.value.length
  const steps = t(`workflow.steps.count.${pluralForm(count, locale.value)}`, { count })
  const data = split.value.data.map(summary).filter(Boolean).join(', ')
  return data ? `${steps} · ${t('workflow.steps.data', { data })}` : steps
})
</script>

<template>
  <div :class="cn('flex flex-col gap-3', !canvas && 'min-h-0 flex-1 overflow-y-auto pb-4')" data-tour="workflow-steps">
    <div class="flex items-center gap-3">
      <p class="min-w-0 flex-1 truncate text-sm text-muted-foreground">{{ meta }}</p>
      <Button variant="outline" class="h-10" data-steps-canvas @click="canvas = !canvas">
        {{ canvas ? t('workflow.steps.list') : t('workflow.steps.canvas') }}
      </Button>
    </div>
    <template v-if="!canvas">
      <ol class="grid gap-2">
        <li v-for="(row, index) in rows" :key="row.id" class="relative">
          <span v-if="index < rows.length - 1" class="absolute top-full left-4 h-2 w-px bg-border" aria-hidden="true" />
          <Button
            variant="outline"
            :class="
              cn(
                'h-auto min-h-14 w-full justify-start gap-3 rounded-lg px-3 py-2.5 text-left font-normal',
                selected === row.id && 'border-primary',
                row.disabled && 'opacity-50',
              )
            "
            :data-step="row.id"
            @click="editor.select(row.id)"
          >
            <span :class="cn('size-2 shrink-0 rounded-full', row.color)" />
            <span class="grid min-w-0 flex-1">
              <span class="truncate text-sm">{{ row.title }}</span>
              <span v-if="row.problem" class="truncate text-xs text-destructive" data-step-problem>{{ row.problem }}</span>
              <span v-else-if="row.value" class="truncate text-xs text-muted-foreground">{{ row.value }}</span>
            </span>
            <span
              v-if="row.order"
              class="grid h-5 min-w-5 shrink-0 place-items-center rounded-md bg-foreground px-1 text-xs font-semibold text-background tabular-nums"
            >
              {{ row.order }}
            </span>
            <ChevronRight v-else class="size-4 shrink-0 text-muted-foreground" />
          </Button>
        </li>
      </ol>
      <Button variant="outline" class="h-11 w-full" data-steps-add @click="emit('add')">
        <Plus class="size-4" />
        {{ t('workflow.steps.add') }}
      </Button>
      <slot />
    </template>
  </div>
</template>
