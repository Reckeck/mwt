<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { type RouteLocationRaw, onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { WorkflowEditor } from '@/widgets/workflow-editor'
import { AutomationWorkflowBanner, useAutomationWorkflow } from '@/features/automation-edit'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'

// `Automations ›` and the name live in the editor's own toolbar (Auto-2 · 13).
const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const workflowId = computed(() => (route.name === 'workflow-edit' ? String(route.params.id) : null))
const template = computed(() => (typeof route.query.template === 'string' ? route.query.template : null))
const lesson = computed(() => (typeof route.query.lesson === 'string' ? route.query.lesson : null))
const editor = ref<InstanceType<typeof WorkflowEditor> | null>(null)
const automation = useAutomationWorkflow(() => workflowId.value)
let leaving = false

/** A new workflow, once saved, gets its own address. */
function onSaved(id: string) {
  if (workflowId.value === id) return
  leaving = true
  void router.replace({ name: 'workflow-edit', params: { id } }).finally(() => {
    leaving = false
  })
}

/* Leaving with unsaved edits asks what to do with them: keep them as a draft,
   throw them away, or stay. */
const leaveTo = ref<RouteLocationRaw | null>(null)
const leaveOpen = computed({
  get: () => leaveTo.value !== null,
  set: (value: boolean) => {
    if (!value) leaveTo.value = null
  },
})
const keeping = ref(false)

// A navigation that did not come through the guard (replace, redirect) settles the question.
watch(() => route.fullPath, () => {
  leaveTo.value = null
})

onBeforeRouteLeave((to) => {
  if (leaving || !editor.value?.dirty) return true
  leaveTo.value = to.fullPath
  return false
})

function go() {
  const target = leaveTo.value
  leaveTo.value = null
  if (!target) return
  leaving = true
  void router.push(target).finally(() => {
    leaving = false
  })
}

async function keepDraft() {
  keeping.value = true
  try {
    await editor.value?.saveDraft()
    go()
  } catch {
    // The toast says why; the dialog stays so nothing is lost.
  } finally {
    keeping.value = false
  }
}

function throwAway() {
  editor.value?.discardEdits()
  go()
}
</script>

<template>
  <div class="flex h-full min-h-[36rem] flex-col gap-3 @container max-md:min-h-0 max-md:gap-0">
    <AutomationWorkflowBanner
      v-if="workflowId && automation.isAutomation.value"
      :workflow-id="workflowId"
      :name="automation.workflow.data.value?.name ?? ''"
    />
    <WorkflowEditor
      class="min-h-0 flex-1"
      :key="workflowId ?? `new-${template ?? ''}-${lesson ?? ''}`"
      ref="editor"
      :workflow-id="workflowId"
      :template="template"
      :lesson="lesson"
      :locked="automation.isAutomation.value"
      @saved="onSaved"
      @lesson="(id) => router.push({ name: 'workflow-new', query: { lesson: id } })"
    />
    <!-- A new workflow has nothing saved to keep running: its own title and text. -->
    <Dialog overlay="workflow-leave"
      v-model:open="leaveOpen"
      :title="editor?.canDraft ? t('workflow.leave.title') : t('workflow.leave.newTitle')"
      :description="editor?.canDraft ? t('workflow.leave.body') : t('workflow.leave.newHint')"
      size="form"
    >
      <template #footer>
        <Button variant="outline" class="max-md:w-full" @click="leaveOpen = false">{{ t('workflow.leave.stay') }}</Button>
        <Button variant="outline" class="max-md:w-full" @click="throwAway">{{ t('workflow.leave.discard') }}</Button>
        <Button v-if="editor?.canDraft" class="max-md:w-full" :disabled="keeping" @click="keepDraft">{{ t('workflow.leave.draft') }}</Button>
      </template>
    </Dialog>
  </div>
</template>
