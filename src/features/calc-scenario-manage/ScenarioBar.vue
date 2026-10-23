<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Copy, Pencil, Target, Trash2 } from 'lucide-vue-next'

import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import type { UiAction } from '@/shared/ui/context-menu'
import { Select, SelectItem } from '@/shared/ui/select'

import { useAutosave } from './model/use-autosave'
import { type ScenarioBody, useScenarios } from './model/use-scenarios'
import ScenarioNameDialog from './ui/ScenarioNameDialog.vue'

/**
 * Saved runs of the calculator: pick one to load it, save a new one, turn it
 * into a goal. A saved one keeps up with the form by itself. The form lives in
 * the page, so the bar reads it through `snapshot` and hands loads back via `load`.
 */
const props = defineProps<{ snapshot: () => ScenarioBody }>()
const scenarioId = defineModel<string | null>('scenarioId', { required: true })
const emit = defineEmits<{
  load: [scenario: CalcScenario]
  goal: [draft: GoalInput, scenarioId: string]
  /** «Создать цель» was pressed: the page settles the form (a found contribution goes in) and calls `createGoal`. */
  wantGoal: []
}>()

const { t } = useI18n()
const { list, scenarios, byId, save, autosave, remove, goalDraft } = useScenarios()
const current = computed(() => byId(scenarioId.value))
const { saving } = useAutosave(current, props.snapshot, autosave)
/** «Создать цель» on an unsaved run asks for a name first, then goes on to the goal. */
const goalAfterSave = ref(false)

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
        if (goalAfterSave.value) makeGoal(saved)
        goalAfterSave.value = false
      },
    },
  )
}

function createGoal() {
  if (current.value) return makeGoal(current.value)
  goalAfterSave.value = true
  naming.value = 'new'
}
defineExpose({ createGoal })

watch(naming, (mode) => {
  if (mode === null && !save.isPending.value) goalAfterSave.value = false
})

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
  <div class="flex flex-wrap items-center gap-x-3 gap-y-2" data-tour="calc-scenarios">
    <Select v-model="picked" class="w-full sm:w-56" :aria-label="t('invest.scenario.pick')">
      <SelectItem :value="NONE">{{ t('invest.scenario.unsaved') }}</SelectItem>
      <SelectItem v-for="scenario in scenarios" :key="scenario.id" :value="scenario.id">{{ scenario.name }}</SelectItem>
    </Select>
    <span class="text-xs text-muted-foreground" aria-live="polite" data-calc-saved>
      {{ !current ? t('invest.scenario.notSaved') : saving ? t('invest.scenario.saving') : t('invest.scenario.autosaved') }}
    </span>
    <div class="ms-auto flex items-center gap-2">
      <Button v-if="!current" variant="outline" :disabled="save.isPending.value" @click="naming = 'new'">
        {{ t('invest.scenario.save') }}
      </Button>
      <ActionMenu v-if="current" :actions="actions" :label="t('actions.more')" />
      <Button :disabled="goalDraft.isPending.value || save.isPending.value" data-calc-goal @click="emit('wantGoal')">
        <Target class="size-4" />{{ t('invest.scenario.makeGoal') }}
      </Button>
    </div>

    <ScenarioNameDialog
      v-model:open="nameOpen"
      :title="nameTitle"
      :initial="nameInitial"
      :pending="save.isPending.value"
      @submit="named"
    />
  </div>
</template>
