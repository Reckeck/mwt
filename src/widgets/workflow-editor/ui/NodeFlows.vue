<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ArrowLeft, ArrowRight, Link2Off, Plus } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'

import type { FlowPortLinks } from '../model/flow'

/**
 * The selected node's flow wires, port by port: where the run comes from and
 * where it goes next. A wire can be cut here without hunting for it on the
 * canvas; an empty output offers the node search right there.
 */
const props = defineProps<{ ports: FlowPortLinks[] }>()
const emit = defineEmits<{
  unlink: [edgeId: string]
  add: [port: string, side: 'in' | 'out', event: MouseEvent]
}>()

const { t } = useI18n()
</script>

<template>
  <section v-if="props.ports.length" class="grid grid-cols-[minmax(0,1fr)] gap-2" data-tour="workflow-node-flows">
    <h4 class="text-sm font-medium">{{ t('workflow.inspector.flows') }}</h4>
    <div v-for="port in props.ports" :key="`${port.side}:${port.port}`" class="grid min-w-0 gap-1">
      <span class="flex items-center gap-1 text-xs text-muted-foreground">
        <component :is="port.side === 'in' ? ArrowLeft : ArrowRight" class="size-3 shrink-0" aria-hidden="true" />
        {{ port.label }}
      </span>
      <div
        v-for="link in port.links"
        :key="link.edgeId"
        class="flex min-w-0 items-center gap-2 rounded-md border border-border px-2 py-1 text-sm"
      >
        <span class="min-w-0 flex-1 truncate" :title="link.title">{{ link.title }}</span>
        <Button
          variant="ghost"
          size="icon-xs"
          :aria-label="t('workflow.inspector.unlinkFlow', { node: link.title })"
          :title="t('workflow.inspector.unlinkFlow', { node: link.title })"
          @click="emit('unlink', link.edgeId)"
        >
          <Link2Off class="size-3.5" />
        </Button>
      </div>
      <Button
        v-if="!port.links.length"
        variant="ghost"
        size="sm"
        class="justify-start border border-dashed border-border font-normal text-muted-foreground"
        @click="(event: MouseEvent) => emit('add', port.port, port.side, event)"
      >
        <Plus class="size-3.5" />
        {{ port.side === 'out' ? t('workflow.inspector.flowEmptyOut') : t('workflow.inspector.flowEmptyIn') }}
      </Button>
    </div>
  </section>
</template>
