<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { CircleCheck, Circle, X } from 'lucide-vue-next'

import { StoreCreateDialog } from '@/features/store-create'
import { Button, buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { call } from '@/shared/api/ipc'
import { STORE_TREE_QUERY_KEY, flattenStoreTree } from '@/shared/lib/store-tree'
import { useSettingsStore } from '@/shared/store/settings'

const { t } = useI18n()
const settings = useSettingsStore()

// Each step is checked off from the vault itself, not by ticking a box.
const tree = useQuery({ queryKey: STORE_TREE_QUERY_KEY, queryFn: () => call<StoreTreeNode[]>('get_store_tree') })
const txs = useQuery({ queryKey: ['transactions', ''], queryFn: () => call<Transaction[]>('list_transactions', { filter: null }) })
const balances = useQuery({
  queryKey: ['contacts', 'balances'],
  queryFn: () => call<ContactBalance[]>('get_contact_balances'),
})

const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations') })
const goals = useQuery({ queryKey: ['goals'], queryFn: () => call<Goal[]>('list_goals') })

const stores = computed(() => flattenStoreTree(tree.data.value ?? []))

const steps = computed(() => [
  { key: 'wallet', done: stores.value.length > 0 },
  {
    key: 'debts',
    done: stores.value.some((node) => node.store.kind === 'liability') || (balances.data.value?.length ?? 0) > 0,
    to: { name: 'stores' },
  },
  { key: 'savings', done: Number(settings.vault?.savings_rate ?? 0.1) !== 0.1, to: { name: 'dashboard', query: { settings: 'budget' } } },
  { key: 'operation', done: (txs.data.value?.length ?? 0) > 0, to: { name: 'stores' } },
  { key: 'salary', done: (automations.data.value?.length ?? 0) > 0, to: { name: 'planning-automations' } },
  { key: 'goal', done: (goals.data.value?.length ?? 0) > 0, to: { name: 'planning-goals' } },
])

const allDone = computed(() => steps.value.every((step) => step.done))

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
</script>

<template>
  <Card v-if="!hidden && !allDone && tree.isSuccess.value">
    <template #header>
      <div class="flex items-center justify-between gap-2">
        <p class="font-medium">{{ t('onboarding.title') }}</p>
        <Button variant="ghost" size="icon" class="size-8" :title="t('onboarding.hide')" :aria-label="t('onboarding.hide')" @click="hide">
          <X class="size-4" />
        </Button>
      </div>
    </template>
    <ol class="grid gap-2">
      <li v-for="(step, index) in steps" :key="step.key" class="flex items-center gap-3 text-sm">
        <component :is="step.done ? CircleCheck : Circle" class="size-4 shrink-0" :class="step.done ? 'text-emerald-600 dark:text-emerald-400' : 'text-muted-foreground'" />
        <span class="min-w-0 flex-1" :class="step.done && 'text-muted-foreground line-through'">
          {{ index + 1 }}. {{ t(`onboarding.step.${step.key}`) }}
        </span>
        <template v-if="!step.done">
          <Button v-if="step.key === 'wallet'" variant="outline" size="sm" @click="createOpen = true">
            {{ t('onboarding.do') }}
          </Button>
          <RouterLink v-else-if="step.to" :to="step.to" :class="buttonVariants({ variant: 'outline', size: 'sm' })">
            {{ t('onboarding.do') }}
          </RouterLink>
        </template>
      </li>
    </ol>
    <StoreCreateDialog v-model:open="createOpen" />
  </Card>
</template>
