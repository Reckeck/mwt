<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Copy, Pencil, Save, Target, Trash2 } from 'lucide-vue-next'

import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import type { UiAction } from '@/shared/ui/context-menu'
import { Select, SelectItem } from '@/shared/ui/select'

import { type ScenarioBody, useScenarios } from './model/use-scenarios'
import ScenarioNameDialog from './ui/ScenarioNameDialog.vue'

/**
 * Saved runs of the calculator: pick one to load it, save the current form
 * over it or as a new one, turn it into a goal. The form itself lives in the
 * page, so the bar reads it through `snapshot` and hands loads back via `load`.
 */
const props = defineProps<{ snapshot: () => ScenarioBody }>()
const scenarioId = defineModel<string | null>('scenarioId', { required: true })
const emit = defineEmits<{ load: [scenario: CalcScenario]; goal: [draft: GoalInput, scenarioId: string] }>()

const { t } = useI18n()
const { list, scenarios, byId, save, remove, goalDraft } = useScenarios()
const current = computed(() => byId(scenarioId.value))

// A scenario named in the address loads once the list arrives, and again on each pick.
const loadedId = ref<string | null>(null)
watch(
  [scenarioId, () => list.data.value],
  () => {
    const scenario = current.value
    if (!scenario || loadedId.value === scenario.id) return
    loadedId.value = scenario.id
    emit('load', scenario)
  },
  { immediate: true },
)

type NameMode = 'new' | 'copy' | 'rename'
const naming = ref<NameMode | null>(null)
const nameOpen = computed({
  get: () => naming.value !== null,
  set: (open) => {
    if (!open) naming.value = null
  },
})
const nameTitle = computed(() => t(`invest.scenario.nameTitle.${naming.value ?? 'new'}`))
const nameInitial = computed(() =>
  naming.value === 'rename' ? (current.value?.name ?? '') : naming.value === 'copy' ? t('invest.scenario.copyOf', { name: current.value?.name ?? '' }) : '',
)

function store(id: string | null, name: string, body: ScenarioBody) {
  save.mutate(
    { id, name, body },
    {
      onSuccess: (saved) => {
        naming.value = null
        loadedId.value = saved.id
        scenarioId.value = saved.id
      },
    },
  )
}

function saveCurrent() {
  if (current.value) store(current.value.id, current.value.name, props.snapshot())
  else naming.value = 'new'
}

function named(name: string) {
  const scenario = current.value
  if (naming.value === 'rename' && scenario) {
    store(scenario.id, name, { input: scenario.input, source: scenario.source, prefilledAt: scenario.prefilled_at })
  } else store(null, name, props.snapshot())
}

function makeGoal(scenario: CalcScenario) {
  goalDraft.mutate(scenario.id, { onSuccess: (draft) => emit('goal', draft, scenario.id) })
}

function drop(scenario: CalcScenario) {
  remove.mutate(scenario.id, {
    onSuccess: () => {
      loadedId.value = null
      scenarioId.value = null
    },
  })
}

const actions = computed<UiAction[]>(() => {
  const scenario = current.value
  if (!scenario) return []
  return [
    { id: 'rename', label: t('invest.scenario.rename'), icon: Pencil, run: () => (naming.value = 'rename') },
    { id: 'copy', label: t('invest.scenario.saveAsNew'), icon: Copy, run: () => (naming.value = 'copy') },
    { id: 'goal', label: t('invest.scenario.makeGoal'), icon: Target, run: () => makeGoal(scenario) },
    { id: 'delete', label: t('actions.delete'), icon: Trash2, destructive: true, run: () => drop(scenario) },
  ]
})

const NONE = '__none'
const picked = computed({
  get: () => scenarioId.value ?? NONE,
  set: (value: string) => {
    scenarioId.value = value === NONE ? null : value
  },
})
</script>

<template>
  <div class="flex flex-wrap items-center gap-2" data-tour="calc-scenarios">
    <Select v-model="picked" class="w-full sm:w-64" :aria-label="t('invest.scenario.pick')">
      <SelectItem :value="NONE">{{ t('invest.scenario.unsaved') }}</SelectItem>
      <SelectItem v-for="scenario in scenarios" :key="scenario.id" :value="scenario.id">{{ scenario.name }}</SelectItem>
    </Select>
    <Button variant="outline" :disabled="save.isPending.value" @click="saveCurrent">
      <Save class="size-4" />
      {{ current ? t('invest.scenario.update') : t('invest.scenario.save') }}
    </Button>
    <ActionMenu v-if="current" :actions="actions" :label="t('actions.more')" />
    <p v-if="!scenarios.length && list.isSuccess.value" class="basis-full text-xs text-muted-foreground">
      {{ t('invest.scenario.emptyHint') }}
    </p>

    <ScenarioNameDialog
      v-model:open="nameOpen"
      :title="nameTitle"
      :initial="nameInitial"
      :pending="save.isPending.value"
      @submit="named"
    />
  </div>
</template>
