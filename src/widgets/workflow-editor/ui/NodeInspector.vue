<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { CircleHelp, Link2Off, Power, Trash2 } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Input } from '@/shared/ui/input'
import { Label } from '@/shared/ui/label'
import { localized } from '@/shared/lib/workflow'

import { paramVisible } from '../model/fields'
import type { EditorNode, FlowPortLinks } from '../model/flow'
import NodeFlows from './NodeFlows.vue'
import ParamField from './ParamField.vue'

/**
 * The selected node: its fields, where wired inputs come from, its flow wires,
 * what is wrong with it, and its help (F1).
 */
const props = defineProps<{
  node: EditorNode
  def: NodeDef | undefined
  /** Input port → where its wire comes from, in words. */
  wiredFrom: Record<string, string>
  /** Flow ports and their wires. */
  flows: FlowPortLinks[]
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
const issueText = (issue: GraphIssue) => (locale.value.startsWith('ru') ? issue.text_ru : issue.text_en)
</script>

<template>
  <div class="grid grid-cols-[minmax(0,1fr)] gap-4 [overflow-wrap:anywhere] [&>*]:min-w-0" data-tour="workflow-inspector">
    <div class="flex items-start gap-2">
      <div class="min-w-0 flex-1">
        <h3 class="font-semibold">{{ props.def ? text(props.def.title) : props.node.data?.kind }}</h3>
        <p v-if="props.def" class="text-sm text-muted-foreground">{{ text(props.def.description) }}</p>
      </div>
      <Button variant="ghost" size="icon" :aria-label="t('workflow.inspector.help')" :aria-pressed="help" @click="help = !help">
        <CircleHelp class="size-4" />
      </Button>
    </div>

    <div v-if="help && props.def" class="rounded-md bg-muted p-3 text-sm" data-tour="workflow-help">
      <p>{{ text(props.def.help) }}</p>
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
        <div class="flex items-center gap-2 rounded-md border border-dashed border-border px-2 py-1.5 text-sm">
          <span class="min-w-0 flex-1 truncate">{{ t('workflow.inspector.fromWire', { source: props.wiredFrom[param.name] }) }}</span>
          <Button variant="ghost" size="icon" :aria-label="t('workflow.inspector.unwire')" @click="emit('unwire', param.name)">
            <Link2Off class="size-4" />
          </Button>
        </div>
      </div>
      <ParamField v-else :param="param" :value="params[param.name]" :params="params" @update="(value) => emit('param', param.name, value)" />
    </template>

    <p v-if="props.def?.params.some((param) => param.wirable)" class="text-xs text-muted-foreground">
      {{ t('workflow.inspector.wireHint') }}
    </p>

    <NodeFlows
      :ports="props.flows"
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
    </div>
  </div>
</template>
