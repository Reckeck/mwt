<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useEventListener } from '@vueuse/core'
import { FileJson, Info } from 'lucide-vue-next'

import { StoreNodeSelect } from '@/entities/store'
import { Alert, AlertDescription } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { Empty, EmptyDescription, EmptyHeader, EmptyMedia, EmptyTitle } from '@/shared/ui/empty'
import { FormField } from '@/shared/ui/form-field'
import { useStoreTree } from '@/shared/api/queries'
import { pluralForm } from '@/shared/lib/format'
import { withoutLoyalty } from '@/shared/lib/store-tree'

import { useWorkflowImport } from '../../model/use-workflow-import'

/** «Импорт автоматизации» (Auto-3 · 17): a .json file dropped or chosen, what it holds, a wallet in place of each missing one. */
const open = defineModel<boolean>('open', { default: false })
const emit = defineEmits<{ imported: [workflow: Workflow] }>()

const { t, locale } = useI18n()
const tree = useStoreTree()
const nodes = computed(() => withoutLoyalty(tree.data.value ?? []))
const zone = ref<HTMLElement | null>(null)
const input = ref<HTMLInputElement | null>(null)
const { dragging, stores, found, missing, form, pick, submit, pending } = useWorkflowImport(open, (workflow) =>
  emit('imported', workflow),
)

function picked(event: Event) {
  const target = event.target as HTMLInputElement
  const file = target.files?.[0]
  target.value = ''
  void pick(file)
}

useEventListener(zone, 'dragover', (event: DragEvent) => {
  event.preventDefault()
  dragging.value = true
})
useEventListener(zone, 'dragleave', () => {
  dragging.value = false
})
useEventListener(zone, 'drop', (event: DragEvent) => {
  event.preventDefault()
  dragging.value = false
  void pick(event.dataTransfer?.files?.[0])
})
</script>

<template>
  <Dialog overlay="workflow-import" v-model:open="open" size="form" :title="t('workflow.import.title')">
    <form id="workflow-import" class="grid gap-4" @submit.prevent="form.submit(submit, $event)">
      <div class="grid gap-1.5">
        <div ref="zone" data-import-zone>
          <Empty :class="['border border-dashed', dragging ? 'border-primary bg-accent' : 'border-border', form.error('file') && 'border-destructive']">
            <EmptyHeader>
              <EmptyMedia variant="icon"><FileJson /></EmptyMedia>
              <EmptyTitle class="text-base">
                {{ t('workflow.import.drop') }}
                <Button type="button" variant="link" class="h-auto p-0 max-md:h-11" @click="input?.click()">{{ t('workflow.import.choose') }}</Button>
              </EmptyTitle>
              <EmptyDescription>{{ t('workflow.import.format') }}</EmptyDescription>
            </EmptyHeader>
          </Empty>
          <input ref="input" type="file" accept=".json,application/json" class="hidden" data-import-input @change="picked" />
        </div>
        <p v-if="form.error('file')" class="text-xs text-destructive" data-import-error>{{ form.error('file') }}</p>
      </div>
      <Alert v-if="found" class="border-0 bg-muted" data-import-found>
        <Info />
        <AlertDescription>
          <p>
            {{ t('workflow.import.found', { name: found.name }) }} —
            {{ t(`workflow.import.steps.${pluralForm(found.steps, locale)}`, { count: found.steps }) }}.
            <template v-for="entry in missing" :key="entry.id">{{ ' ' + t('workflow.import.missing', { name: entry.name }) }}</template>
          </p>
        </AlertDescription>
      </Alert>
      <FormField
        v-for="entry in missing"
        :key="entry.id"
        :for="`import-store-${entry.id}`"
        :label="t('workflow.import.instead', { name: entry.name })"
        required
        :error="form.error(`store-${entry.id}`)"
      >
        <StoreNodeSelect :id="`import-store-${entry.id}`" v-model="stores[entry.id]" :nodes="nodes" class="w-full" />
      </FormField>
    </form>
    <template #footer>
      <Button variant="ghost" class="max-md:h-11" @click="open = false">{{ t('actions.cancel') }}</Button>
      <Button class="max-md:h-11 max-md:flex-1" form="workflow-import" type="submit" :disabled="pending">{{ t('workflow.import.submit') }}</Button>
    </template>
  </Dialog>
</template>
