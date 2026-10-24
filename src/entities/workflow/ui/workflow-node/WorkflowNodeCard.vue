<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { Handle, Position } from '@vue-flow/core'
import { AlertTriangle, CircleSlash } from 'lucide-vue-next'

import { cn } from '@/shared/lib/utils'
import { GROUP_COLOR, localized, portStroke } from '@/shared/lib/workflow'

import type { WorkflowNodeData } from './workflow-node.types'

/**
 * One node on the canvas (Auto-2 · 13): a dot of its group's colour and the
 * title, its value in words under them, and a row per wired port — inputs on
 * the left, outputs on the right. Every port shows on a hovered node, or
 * one with no wires yet. Flow ports are squares, data ports dots.
 */
const props = defineProps<{ id: string; data: WorkflowNodeData; selected?: boolean }>()

const { t, locale } = useI18n()
const text = (value: LocalizedText) => localized(value, locale.value)

const title = computed(() => (props.data.def ? text(props.data.def.title) : props.data.kind))
const errors = computed(() => props.data.issues.filter((issue) => issue.level === 'error'))
const issueText = computed(() =>
  props.data.issues.map((issue) => (locale.value.startsWith('ru') ? issue.text_ru : issue.text_en)).join('\n'),
)
/** What the emulated step did, in words: the run number's tooltip. */
const stepText = computed(() => {
  const step = props.data.step
  return step ? (locale.value.startsWith('ru') ? step.text_ru : step.text_en) : undefined
})
const hovered = ref(false)
const showAll = computed(() => hovered.value || !props.data.linked.length)
const rows = computed(() => {
  const shown = (side: string) => (port: PortDef) => showAll.value || props.data.linked.includes(`${side}:${port.name}`)
  const inputs = (props.data.def?.inputs ?? []).filter(shown('in'))
  const outputs = (props.data.def?.outputs ?? []).filter(shown('out'))
  return Array.from({ length: Math.max(inputs.length, outputs.length) }, (_, index) => ({
    input: inputs[index] ?? null,
    output: outputs[index] ?? null,
  }))
})
const stepClass = computed(() => {
  const status = props.data.step?.status
  if (!status) return ''
  if (status === 'error') return 'ring-2 ring-destructive'
  return status === 'skipped' ? 'opacity-50' : ''
})
const dot = (type: PortType) =>
  cn('!size-2.5 !border-2 !border-card', type === 'flow' ? '!size-3 !rounded-sm' : '!rounded-full')
/** The port's colour inline: Vue Flow's own handle style would win over a class. */
const dotStyle = (type: PortType) => ({ background: portStroke(type) })
// check-ui-ignore fixed-width: a node on the canvas, which pans and zooms
</script>

<template>
  <!-- biome-ignore lint/a11y/noStaticElementInteractions: hover only reveals the unwired ports; Vue Flow handles the node's clicks and keys. -->
  <div
    :class="
      cn(
        showAll ? 'w-64' : 'w-52',
        'relative rounded-lg border bg-card text-card-foreground shadow-sm transition-shadow',
        props.selected ? 'border-primary shadow-md ring-2 ring-primary/30' : 'border-border',
        errors.length ? 'border-2 border-destructive' : '',
        props.data.disabled ? 'opacity-50' : '',
        stepClass,
      )
    "
    :title="issueText || undefined"
    @mouseenter="hovered = true"
    @mouseleave="hovered = false"
  >
    <span
      v-if="props.data.order"
      class="absolute -top-2.5 -right-2.5 grid h-5 min-w-5 place-items-center rounded-md bg-foreground px-1 text-xs font-semibold text-background tabular-nums"
      :title="stepText"
      data-node-order
    >
      {{ props.data.order }}
    </span>
    <span v-if="props.data.problem" class="absolute bottom-full left-0 mb-1 max-w-full truncate text-xs text-destructive" data-node-problem>
      {{ props.data.problem }}
    </span>
    <div class="flex items-center gap-2 border-b border-border px-3 py-2">
      <span :class="cn('size-2 shrink-0 rounded-full', props.data.def ? GROUP_COLOR[props.data.def.group] : 'bg-muted-foreground')" />
      <span class="min-w-0 flex-1 truncate text-sm font-semibold">{{ props.data.label || title }}</span>
      <CircleSlash v-if="props.data.disabled" class="size-4 shrink-0 text-muted-foreground" :aria-label="t('workflow.node.disabled')" />
      <AlertTriangle
        v-if="props.data.issues.length"
        :class="cn('size-4 shrink-0', errors.length ? 'text-destructive' : 'text-muted-foreground')"
        :aria-label="issueText"
      />
    </div>
    <p v-if="props.data.summary" class="truncate px-3 pt-2 text-sm" :title="props.data.summary">{{ props.data.summary }}</p>
    <div class="grid gap-1 py-2">
      <div v-for="(row, index) in rows" :key="index" class="relative flex min-h-5 items-center justify-between gap-2 px-3 text-xs text-muted-foreground">
        <span v-if="row.input" class="truncate">
          <Handle :id="row.input.name" type="target" :position="Position.Left" :class="dot(row.input.type)" :style="dotStyle(row.input.type)" />
          {{ text(row.input.label) }}
        </span>
        <span v-else />
        <span v-if="row.output" class="truncate text-right">
          {{ text(row.output.label) }}
          <Handle :id="row.output.name" type="source" :position="Position.Right" :class="dot(row.output.type)" :style="dotStyle(row.output.type)" />
        </span>
      </div>
    </div>
  </div>
</template>
