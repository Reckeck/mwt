<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Handle, Position } from '@vue-flow/core'
import { AlertTriangle, CircleSlash } from 'lucide-vue-next'

import { cn } from '@/shared/lib/utils'
import { GROUP_COLOR, localized, portStroke } from '@/shared/lib/workflow'

import type { WorkflowNodeData } from './workflow-node.types'

/**
 * One node on the canvas: a coloured stripe for its group, its title, and a
 * row per port — inputs on the left, outputs on the right. Flow ports are
 * squares, data ports dots coloured by type.
 */
const props = defineProps<{ id: string; data: WorkflowNodeData; selected?: boolean }>()

const { t, locale } = useI18n()
const text = (value: LocalizedText) => localized(value, locale.value)

const title = computed(() => (props.data.def ? text(props.data.def.title) : props.data.kind))
const errors = computed(() => props.data.issues.filter((issue) => issue.level === 'error'))
const issueText = computed(() =>
  props.data.issues.map((issue) => (locale.value.startsWith('ru') ? issue.text_ru : issue.text_en)).join('\n'),
)
const rows = computed(() => {
  const inputs = props.data.def?.inputs ?? []
  const outputs = props.data.def?.outputs ?? []
  return Array.from({ length: Math.max(inputs.length, outputs.length) }, (_, index) => ({
    input: inputs[index] ?? null,
    output: outputs[index] ?? null,
  }))
})
const stepClass = computed(() => {
  const status = props.data.step?.status
  if (!status) return ''
  if (status === 'error') return 'ring-2 ring-destructive'
  if (status === 'skipped') return 'opacity-60'
  return 'ring-2 ring-primary/60'
})
const dot = (type: PortType) =>
  cn('!size-2.5 !border-2 !border-card', type === 'flow' ? '!size-3 !rounded-sm' : '!rounded-full')
/** The port's colour inline: Vue Flow's own handle style would win over a class. */
const dotStyle = (type: PortType) => ({ background: portStroke(type) })
// check-ui-ignore fixed-width: a node on the canvas, which pans and zooms
</script>

<template>
  <div
    :class="
      cn(
        'w-60 rounded-lg border bg-card text-card-foreground shadow-sm transition-shadow',
        props.selected ? 'border-primary shadow-md' : 'border-border',
        errors.length ? 'border-destructive' : '',
        props.data.disabled ? 'opacity-50' : '',
        stepClass,
      )
    "
    :title="issueText || undefined"
  >
    <div :class="cn('h-1.5 rounded-t-lg', props.data.def ? GROUP_COLOR[props.data.def.group] : 'bg-muted')" />
    <div class="flex items-start gap-2 px-3 pt-2">
      <div class="min-w-0 flex-1">
        <div class="truncate text-sm font-medium">{{ props.data.label || title }}</div>
        <div v-if="props.data.label" class="truncate text-xs text-muted-foreground">{{ title }}</div>
      </div>
      <CircleSlash v-if="props.data.disabled" class="mt-0.5 size-4 shrink-0 text-muted-foreground" :aria-label="t('workflow.node.disabled')" />
      <AlertTriangle
        v-if="props.data.issues.length"
        :class="cn('mt-0.5 size-4 shrink-0', errors.length ? 'text-destructive' : 'text-muted-foreground')"
        :aria-label="issueText"
      />
    </div>
    <p v-if="props.data.summary" class="truncate px-3 text-xs text-muted-foreground">{{ props.data.summary }}</p>
    <div class="grid gap-1 py-2">
      <div v-for="(row, index) in rows" :key="index" class="relative flex min-h-5 items-center justify-between gap-2 px-3 text-xs">
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
    <p
      v-if="props.data.step"
      class="truncate border-t border-border px-3 py-1.5 text-xs text-muted-foreground"
      :title="locale.startsWith('ru') ? props.data.step.text_ru : props.data.step.text_en"
    >
      {{ locale.startsWith('ru') ? props.data.step.text_ru : props.data.step.text_en }}
    </p>
  </div>
</template>
