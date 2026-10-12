<script setup lang="ts">
import { computed, ref } from 'vue'
import { onBeforeRouteLeave, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'

import { WorkflowEditor } from '@/widgets/workflow-editor'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

const workflowId = computed(() => (route.name === 'workflow-edit' ? String(route.params.id) : null))
const template = computed(() => (typeof route.query.template === 'string' ? route.query.template : null))
const lesson = computed(() => (typeof route.query.lesson === 'string' ? route.query.lesson : null))
const editor = ref<InstanceType<typeof WorkflowEditor> | null>(null)
let leavingAfterSave = false

/** A new workflow, once saved, gets its own address. */
function onSaved(id: string) {
  if (workflowId.value === id) return
  leavingAfterSave = true
  void router.replace({ name: 'workflow-edit', params: { id } }).finally(() => {
    leavingAfterSave = false
  })
}

onBeforeRouteLeave(() => {
  if (leavingAfterSave || !editor.value?.dirty) return true
  return window.confirm(t('workflow.leaveConfirm'))
})
</script>

<template>
  <div class="h-[calc(100dvh-7rem)] min-h-[36rem] @container">
    <WorkflowEditor
      :key="workflowId ?? `new-${template ?? ''}-${lesson ?? ''}`"
      ref="editor"
      :workflow-id="workflowId"
      :template="template"
      :lesson="lesson"
      @saved="onSaved"
      @lesson="(id) => router.push({ name: 'workflow-new', query: { lesson: id } })"
    />
  </div>
</template>
