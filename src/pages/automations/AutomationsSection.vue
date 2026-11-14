<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { Download, Plus, Variable } from 'lucide-vue-next'

import { WorkflowImportDialog } from '@/features/workflow-manage'
import { WorkflowVars } from '@/widgets/workflow-vars'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { Sheet, SheetContent, SheetDescription, SheetHeader, SheetTitle } from '@/shared/ui/sheet'
import type { UiAction } from '@/shared/ui/context-menu'
import { RouteTabs } from '@/shared/ui/route-tabs'
import { call } from '@/shared/api/ipc'
import { useIsPhone } from '@/shared/lib/phone'
import { WORKFLOW_KEYS } from '@/shared/lib/workflow'

import { MAIN_TABS, MORE_TABS, TAB_ROUTE } from './model/automation-tabs'
import { MINE_KEY } from './model/use-mine'
import { NewAutomation } from './ui/new-automation'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()

/** The journal's tab shows how many wait for you: questions, and operations left from an older app. */
const queue = useQuery({ queryKey: WORKFLOW_KEYS.queue, queryFn: () => call<QueuedActions[]>('list_workflow_queue') })
const waits = useQuery({ queryKey: WORKFLOW_KEYS.waits, queryFn: () => call<WorkflowWait[]>('list_workflow_waits') })
const waiting = computed(
  () => (waits.data.value ?? []).filter((wait) => wait.kind === 'confirm').length + (queue.data.value?.length ?? 0),
)

const label = (tab: string) =>
  tab === 'journal' && waiting.value ? `${t('automations.tab.journal')} (${waiting.value})` : t(`automations.tab.${tab}`)
const tabs = computed(() => MAIN_TABS.map((tab) => ({ name: TAB_ROUTE[tab], label: label(tab) })))
const more = computed(() =>
  MORE_TABS.map((tab) => ({ name: TAB_ROUTE[tab], label: label(tab), hint: t(`automations.hint.${tab}`) })),
)
const phone = useIsPhone()
/** `Import` and `Create` belong to `Mine` (Auto-1 · 1), on a phone too; the empty one offers only `Create` (Auto-1 · 2). */
const onMine = computed(() => route.name === TAB_ROUTE.mine)
const mine = useQuery({ queryKey: MINE_KEY, queryFn: () => call<MineItem[]>('list_my_automations'), enabled: onMine })
const importOpen = ref(false)
const create = () => void router.push({ query: { ...route.query, new: 'blank' } })

/** Variables open over any tab; `?vars=1` keeps the panel across Back and a reload. */
const varsOpen = computed({
  get: () => route.query.vars === '1',
  set: (open: boolean) => void router.replace({ query: { ...route.query, vars: open ? '1' : undefined } }),
})

/** The phone's «⋯»: the secondary actions; `Create` stays a button beside it. */
const phoneActions = computed<UiAction[]>(() => [
  { id: 'vars', label: t('workflow.tab.vars'), icon: Variable, run: () => (varsOpen.value = true) },
  {
    id: 'import',
    label: t('automations.mine.import'),
    icon: Download,
    hidden: !(onMine.value && mine.data.value?.length),
    run: () => (importOpen.value = true),
  },
])
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <!-- A phone: one row, the tabs take the free width, «⋯» holds the secondary actions, «+» stays in sight. -->
    <div v-if="phone" class="flex items-center gap-2">
      <div class="min-w-0 flex-1" data-tour="section-tabs"><RouteTabs :tabs="tabs" :more="more" :more-label="t('nav.more')" /></div>
      <ActionMenu :actions="phoneActions" :label="t('automations.actions')" />
      <Button v-if="onMine" size="sm" class="size-11" :aria-label="t('automations.mine.create')" data-mine-create data-tour="automations-create" @click="create">
        <Plus class="size-4" />
      </Button>
    </div>
    <div v-else class="flex flex-wrap items-center justify-between gap-3">
      <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" :more="more" :more-label="t('nav.more')" /></div>
      <div class="flex items-center gap-2">
        <Button variant="outline" size="sm" :aria-label="t('workflow.tab.vars')" data-tour="workflow-vars-open" @click="varsOpen = true">
          <Variable class="size-4" />
          {{ t('workflow.tab.vars') }}
        </Button>
        <template v-if="onMine">
          <Button v-if="mine.data.value?.length" variant="outline" size="sm" data-mine-import data-tour="automations-import" @click="importOpen = true">
            <Download class="size-4" />
            {{ t('automations.mine.import') }}
          </Button>
          <Button size="sm" :aria-label="t('automations.mine.create')" data-mine-create data-tour="automations-create" @click="create">
            <Plus class="size-4" />
            {{ t('automations.mine.create') }}
          </Button>
        </template>
      </div>
    </div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
    <NewAutomation />
    <WorkflowImportDialog v-model:open="importOpen" @imported="(workflow) => router.push({ name: 'workflow-edit', params: { id: workflow.id } })" />
    <Sheet v-model:open="varsOpen">
      <SheetContent overlay="workflow-vars" side="right" class="w-full overflow-y-auto sm:max-w-2xl">
        <SheetHeader>
          <SheetTitle>{{ t('workflow.tab.vars') }}</SheetTitle>
          <SheetDescription>{{ t('workflow.vars.sheetHint') }}</SheetDescription>
        </SheetHeader>
        <div class="px-4 pb-4"><WorkflowVars /></div>
      </SheetContent>
    </Sheet>
  </div>
</template>
