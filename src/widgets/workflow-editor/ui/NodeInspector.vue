<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { Power, Trash2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Kbd } from '@/shared/ui/kbd'
import { Label } from '@/shared/ui/label'
import { localized } from '@/shared/lib/workflow'

import { paramVisible } from '../model/fields'
import type { EditorNode, FlowPortLinks } from '../model/flow'
import NodeFlows from './NodeFlows.vue'
import ParamField from './ParamField.vue'

/**
 * The selected node (Auto-2 · 13): its fields, where wired inputs come from,
 * when it runs, what is wrong with it, and its help (F1). A flow port shows
 * here only while it has no wire: the button adds the next node.
 */
const props = defineProps<{
  node: EditorNode
  def: NodeDef | undefined
  /** Input port → where its wire comes from, in words. */
  wiredFrom: Record<string, string>
  /** Flow ports and their wires. */
  flows: FlowPortLinks[]
  /** The actions it runs after by data. */
  dataAfter: string[]
  /** «Выполнится: после … , когда есть …». */
  runsWhen: { after: string[]; ready: string[] } | null
  issues: GraphIssue[]
}>()
const help = defineModel<boolean>('help', { default: false })
const emit = defineEmits<{
  param: [name: string, value: unknown]
  label: [value: string]
  toggle: []
  unwire: [port: string]
  unlink: [edgeId: string]
  addFlow: [port: string, side: 'in' | 'out', event: MouseEvent]
  remove: []
}>()

const { t, locale } = useI18n()
const text = (value: LocalizedText) => localized(value, locale.value)
const params = computed(() => props.node.data?.params ?? {})
const fields = computed(() => (props.def?.params ?? []).filter((param) => paramVisible(props.def?.kind ?? '', param.name, params.value)))
const emptyFlows = computed(() => props.flows.filter((port) => !port.links.length))
const runs = computed(() => {
  const when = props.runsWhen
  if (!when) return ''
  const parts = [
    when.after.length ? t('workflow.inspector.runsAfterNodes', { nodes: when.after.join(', ') }) : '',
    when.ready.length ? t('workflow.inspector.runsReady', { fields: when.ready.join(', ') }) : '',
  ]
  return parts.filter(Boolean).join(', ')
})
const issueText = (issue: GraphIssue) => (locale.value.startsWith('ru') ? issue.text_ru : issue.text_en)
/** An empty required field is told under the field itself, not only in the list above. */
const fieldError = (name: string) => {
  const issue = props.issues.find((entry) => entry.param === name)
  return issue ? t('workflow.inspector.fillIn') : null
}
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)] gap-4 [overflow-wrap:anywhere] [&>*]:min-w-0" data-tour="workflow-inspector">
    <h3 class="font-semibold">{{ props.def ? text(props.def.title) : props.node.data?.kind }}</h3>

    <div v-if="help && props.def" class="rounded-md bg-muted p-3 text-sm" data-tour="workflow-help">
      <p class="text-muted-foreground">{{ text(props.def.description) }}</p>
      <p class="mt-2">{{ text(props.def.help) }}</p>
      <dl v-if="props.def.outputs.length" class="mt-2 grid gap-0.5 text-xs text-muted-foreground">
        <div v-for="port in props.def.outputs" :key="port.name">
          <dt class="inline font-medium">{{ text(port.label) }}</dt>
          <dd class="inline"> — {{ t(`workflow.port.${port.type}`) }}</dd>
        </div>
      </dl>
    </div>

    <ul v-if="props.issues.length" class="grid grid-cols-[minmax(0,1fr)] gap-1 [&>*]:min-w-0">
      <li
        v-for="(issue, index) in props.issues"
        :key="index"
        :class="issue.level === 'error' ? 'text-destructive' : 'text-muted-foreground'"
        class="text-xs"
      >
        {{ issueText(issue) }}
      </li>
    </ul>

    <div class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0">
      <Label for="workflow-node-label">{{ t('workflow.inspector.label') }}</Label>
      <Input
        id="workflow-node-label"
        :model-value="props.node.data?.label ?? ''"
        :placeholder="props.def ? text(props.def.title) : ''"
        @update:model-value="(value) => emit('label', String(value))"
      />
    </div>

    <template v-for="param in fields" :key="param.name">
      <div v-if="props.wiredFrom[param.name]" class="grid grid-cols-[minmax(0,1fr)] gap-1.5 [&>*]:min-w-0">
        <span class="text-sm font-medium">{{ text(param.label) }}</span>
        <p class="truncate rounded-md border border-input px-3 py-2 text-sm" data-wired-field>↳ {{ props.wiredFrom[param.name] }}</p>
        <Button variant="link" size="sm" class="h-auto justify-start p-0 text-xs font-normal text-muted-foreground" @click="emit('unwire', param.name)">
          {{ t('workflow.inspector.unwire') }}
        </Button>
      </div>
      <ParamField
        v-else
        :param="param"
        :value="params[param.name]"
        :params="params"
        :error="fieldError(param.name)"
        @update="(value) => emit('param', param.name, value)"
      />
    </template>

    <p v-if="props.def?.params.some((param) => param.wirable)" class="text-xs text-muted-foreground">
      {{ t('workflow.inspector.wireHint') }}
    </p>

    <p v-if="runs" class="border-t border-border pt-3 text-xs text-muted-foreground" data-node-runs>{{ t('workflow.inspector.runs') }} {{ runs }}</p>

    <NodeFlows
      :ports="emptyFlows"
      :after="[]"
      @unlink="(edgeId) => emit('unlink', edgeId)"
      @add="(port, side, event) => emit('addFlow', port, side, event)"
    />

    <div class="flex flex-wrap gap-2">
      <Button variant="outline" size="sm" @click="emit('toggle')">
        <Power class="size-4" />
        {{ props.node.data?.disabled ? t('workflow.inspector.enable') : t('workflow.inspector.disable') }}
      </Button>
      <Button variant="outline" size="sm" class="text-destructive" @click="emit('remove')">
        <Trash2 class="size-4" />
        {{ t('workflow.inspector.remove') }}
      </Button>
      <Button
        variant="ghost"
        size="sm"
        class="ml-auto px-1.5"
        :aria-label="t('workflow.inspector.help')"
        :aria-pressed="help"
        :title="t('workflow.inspector.help')"
        @click="help = !help"
      >
        <Kbd>F1</Kbd>
      </Button>
    </div>
  </div>
</template>
