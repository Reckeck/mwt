<script setup lang="ts">
import { computed, ref } from 'vue'
import { type RouteLocationRaw, onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { WorkflowEditor } from '@/widgets/workflow-editor'
import { Button } from '@/shared/ui/button'
import { Dialog } from '@/shared/ui/dialog'
import { type PageCrumb, PageHeader } from '@/shared/ui/page-header'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const workflowId = computed(() => (route.name === 'workflow-edit' ? String(route.params.id) : null))
const template = computed(() => (typeof route.query.template === 'string' ? route.query.template : null))
const lesson = computed(() => (typeof route.query.lesson === 'string' ? route.query.lesson : null))
const editor = ref<InstanceType<typeof WorkflowEditor> | null>(null)
let leaving = false

/** Back to the list goes through the same «unsaved edits» question as any other way out. */
const crumbs = computed<PageCrumb[]>(() => [
  { label: t('nav.workflows'), to: { name: 'workflows' } },
  { label: lesson.value ? t('workflow.crumb.lesson') : workflowId.value ? t('workflow.crumb.edit') : t('workflow.crumb.new') },
])

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
  <div class="flex h-[calc(100dvh-7rem)] min-h-[36rem] flex-col gap-3 @container">
    <PageHeader :crumbs="crumbs" />
    <WorkflowEditor
      class="min-h-0 flex-1"
      :key="workflowId ?? `new-${template ?? ''}-${lesson ?? ''}`"
      ref="editor"
      :workflow-id="workflowId"
      :template="template"
      :lesson="lesson"
      @saved="onSaved"
      @lesson="(id) => router.push({ name: 'workflow-new', query: { lesson: id } })"
    />
    <Dialog v-model:open="leaveOpen" :title="t('workflow.leave.title')" :description="t('workflow.leave.body')" size="sm">
      <p v-if="!editor?.canDraft" class="text-sm text-muted-foreground">{{ t('workflow.leave.newHint') }}</p>
      <template #footer>
        <Button variant="ghost" @click="leaveOpen = false">{{ t('workflow.leave.stay') }}</Button>
        <Button variant="outline" @click="throwAway">{{ t('workflow.leave.discard') }}</Button>
        <Button v-if="editor?.canDraft" :disabled="keeping" @click="keepDraft">{{ t('workflow.leave.draft') }}</Button>
      </template>
    </Dialog>
  </div>
</template>
