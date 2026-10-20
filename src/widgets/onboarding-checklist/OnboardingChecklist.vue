<script setup lang="ts">
import { computed, ref } from 'vue'
import { type RouteLocationRaw, RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { CircleCheck, Circle, CircleMinus } from 'lucide-vue-next'

import { StoreCreateDialog } from '@/features/store-create'
import { Button, buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { quickAddOpen } from '@/shared/lib/quick-add'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'
import { useStoreTree } from '@/shared/api/queries'

const { t } = useI18n()
const settings = useSettingsStore()

// Each step is checked off from the vault itself, not by ticking a box.
const tree = useStoreTree()
const balances = useQuery({
  queryKey: ['contacts', 'balances'],
  queryFn: () => call<ContactBalance[]>('get_contact_balances'),
})
const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations') })
const goals = useQuery({ queryKey: ['goals'], queryFn: () => call<Goal[]>('list_goals') })

const stores = computed(() => flattenStoreTree(tree.data.value ?? []))
/** The tree already counts each wallet's transactions: no need to load the journal. */
const txCount = computed(() => stores.value.reduce((sum, node) => sum + node.own_tx_count, 0))
const skipped = computed(() => settings.vault?.onboarding_skipped ?? [])

type StepAction = 'createWallet' | 'quickAdd' | { to: RouteLocationRaw }
interface Step {
  key: string
  done: boolean
  action: StepAction
}

const steps = computed<Step[]>(() => [
  { key: 'wallet', done: stores.value.length > 0, action: 'createWallet' },
  {
    key: 'debts',
    done: stores.value.some((node) => node.store.kind === 'liability') || (balances.data.value?.length ?? 0) > 0,
    action: { to: { name: 'stores' } },
  },
  {
    key: 'savings',
    done: settings.vault?.budget_configured === true,
    action: { to: { name: 'dashboard', query: { settings: 'budget' } } },
  },
  { key: 'operation', done: txCount.value > 0, action: 'quickAdd' },
  {
    key: 'salary',
    done: (automations.data.value?.length ?? 0) > 0,
    action: { to: { name: 'planning-automations', query: { new: 'salary' } } },
  },
  { key: 'goal', done: (goals.data.value?.length ?? 0) > 0, action: { to: { name: 'planning-goals' } } },
])

/** A wallet is the one step everything else needs: it cannot be skipped. */
const canSkip = (step: Step) => step.key !== 'wallet'
const isSkipped = (step: Step) => !step.done && skipped.value.includes(step.key)
const finished = computed(() => steps.value.filter((step) => step.done || isSkipped(step)).length)
const left = computed(() => steps.value.length - finished.value)
const allDone = computed(() => left.value === 0)

/** Past half way the list folds into one line; the user can still open it. */
const expanded = ref(false)
const folded = computed(() => !expanded.value && finished.value * 2 >= steps.value.length && !allDone.value)

async function skip(step: Step) {
  try {
    await settings.updateVault({ onboarding_skipped: [...skipped.value, step.key] })
  } catch (err) {
    notify.error(err, 'onboarding.skipFailed')
  }
}

/** Hiding is a per-viewer convenience, so browser storage is enough. */
const HIDDEN_KEY = 'onboarding-hidden'
function readHidden(): boolean {
  try {
    return localStorage.getItem(HIDDEN_KEY) === '1'
  } catch {
    return false
  }
}
const hidden = ref(readHidden())
function hide() {
  hidden.value = true
  try {
    localStorage.setItem(HIDDEN_KEY, '1')
  } catch {
    // Storage blocked: it hides for this session only.
  }
}

const createOpen = ref(false)
function run(action: StepAction) {
  if (action === 'createWallet') createOpen.value = true
  else if (action === 'quickAdd') quickAddOpen.value = true
}
</script>

<template>
  <Card size="sm" v-if="!hidden && tree.isSuccess.value">
    <template #header>
      <!-- In a dashboard frame this row goes: the frame names the widget. -->
      <p data-widget-title class="font-medium">{{ t('onboarding.title') }}</p>
    </template>

    <div v-if="allDone" class="grid gap-2 text-sm">
      <p class="flex items-center gap-2 font-medium">
        <CircleCheck class="size-4 text-success" />
        {{ t('onboarding.allDone') }}
      </p>
      <Button variant="outline" size="sm" class="w-fit" @click="hide">{{ t('onboarding.removeWidget') }}</Button>
    </div>

    <div v-else class="grid gap-3">
      <p class="text-xs text-muted-foreground">{{ t('onboarding.progress', { done: finished, total: steps.length }) }}</p>

      <div v-if="folded" class="flex items-center justify-between gap-2 text-sm">
        <span>{{ t('onboarding.left', { n: left }) }}</span>
        <Button variant="outline" size="sm" @click="expanded = true">{{ t('onboarding.showSteps') }}</Button>
      </div>

      <ol v-else class="grid gap-2">
        <li v-for="(step, index) in steps" :key="step.key" class="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm">
          <component
            :is="step.done ? CircleCheck : isSkipped(step) ? CircleMinus : Circle"
            class="size-4 shrink-0"
            :class="step.done ? 'text-success' : 'text-muted-foreground'"
          />
          <span class="min-w-0 flex-1" :class="(step.done || isSkipped(step)) && 'text-muted-foreground line-through'">
            {{ index + 1 }}. {{ t(`onboarding.step.${step.key}`) }}
          </span>
          <div v-if="!step.done && !isSkipped(step)" class="flex gap-1">
            <Button v-if="canSkip(step)" variant="ghost" size="sm" @click="skip(step)">{{ t('onboarding.skip') }}</Button>
            <RouterLink
              v-if="typeof step.action === 'object'"
              :to="step.action.to"
              :class="buttonVariants({ variant: 'outline', size: 'sm' })"
            >
              {{ t('onboarding.do') }}
            </RouterLink>
            <Button v-else variant="outline" size="sm" @click="run(step.action)">{{ t('onboarding.do') }}</Button>
          </div>
        </li>
      </ol>

      <Button variant="ghost" size="sm" class="w-fit text-muted-foreground" @click="hide">{{ t('onboarding.hide') }}</Button>
    </div>
    <StoreCreateDialog v-model:open="createOpen" />
  </Card>
</template>
