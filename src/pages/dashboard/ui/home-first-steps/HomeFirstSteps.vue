<script setup lang="ts">
import { computed, ref } from 'vue'
import { RouterLink } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { useMutation, useQuery } from '@tanstack/vue-query'
import { Check, Plus, ShieldCheck, Wallet, Zap } from 'lucide-vue-next'

import { StoreCreateDialog } from '@/features/store-create'
import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { quickAddOpen } from '@/shared/lib/quick-add'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { cn } from '@/shared/lib/utils'
import { useSettingsStore } from '@/shared/store/settings'
import { Button, buttonVariants } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'

/**
 * `First steps` on an empty Home (Home 5): four steps checked off from the vault
 * itself; only the next one has a button. Hidden in the vault, so every device agrees.
 */
const { t } = useI18n()
const settings = useSettingsStore()
const tree = useStoreTree()
const automations = useQuery({ queryKey: ['automations'], queryFn: () => call<Automation[]>('list_automations') })

const stores = computed(() => flattenStoreTree(tree.data.value ?? []))
const txCount = computed(() => stores.value.reduce((sum, node) => sum + node.own_tx_count, 0))

const ICONS = { vault: ShieldCheck, wallet: Wallet, operation: Plus, salary: Zap } as const

type Step = { key: 'vault' | 'wallet' | 'operation' | 'salary'; done: boolean; hint: string }
const steps = computed<Step[]>(() => [
  {
    key: 'vault',
    done: true,
    hint: t(settings.cryptoEnabled ? 'home.steps.vaultCrypto' : 'home.steps.vaultPlain', {
      currency: settings.baseCurrency.toUpperCase(),
    }),
  },
  { key: 'wallet', done: stores.value.length > 0, hint: t('home.steps.walletHint') },
  { key: 'operation', done: txCount.value > 0, hint: t('home.steps.operationHint') },
  { key: 'salary', done: (automations.data.value?.length ?? 0) > 0, hint: t('home.steps.salaryHint') },
])
const done = computed(() => steps.value.filter((step) => step.done).length)
const next = computed(() => steps.value.find((step) => !step.done)?.key ?? null)
const shown = computed(
  () => tree.isSuccess.value && next.value !== null && settings.vault?.onboarding_checklist_hidden !== true,
)

const hide = useMutation({
  mutationFn: () => settings.updateVault({ onboarding_checklist_hidden: true }),
  meta: {
    success: 'onboarding.hiddenDone',
    error: 'onboarding.hideFailed',
    undo: () => settings.updateVault({ onboarding_checklist_hidden: false }),
  },
})
const createOpen = ref(false)
function start(key: Step['key']) {
  if (key === 'wallet') createOpen.value = true
  else quickAddOpen.value = true
}
</script>

<template>
  <Card v-if="shown" data-tour="home-first-steps">
    <div class="-mb-1 grid">
      <div class="flex items-center gap-2 pb-1">
        <h2 class="flex-1 text-base font-semibold">{{ t('home.steps.title', { done, total: steps.length }) }}</h2>
        <Button variant="ghost" size="sm" class="-my-1 text-muted-foreground max-md:h-10" @click="hide.mutate()">
          {{ t('home.steps.hide') }}
        </Button>
      </div>
      <div v-for="step in steps" :key="step.key" class="flex min-h-13 items-center gap-3 border-t border-border py-2.5">
        <span
          :class="cn('grid size-8 shrink-0 place-items-center rounded-full bg-secondary', step.done && 'text-success')"
          aria-hidden="true"
        >
          <Check v-if="step.done" class="size-4" />
          <component :is="ICONS[step.key]" v-else class="size-4 text-muted-foreground" />
        </span>
        <div class="grid min-w-0 flex-1">
          <p :class="cn('text-sm font-medium', step.done && 'text-muted-foreground line-through')">
            {{ t(`home.steps.${step.key}`) }}
          </p>
          <p class="truncate text-xs text-muted-foreground">{{ step.hint }}</p>
        </div>
        <template v-if="step.key === next">
          <Button v-if="step.key !== 'salary'" size="sm" class="max-md:h-10" @click="start(step.key)">
            {{ t(`home.steps.${step.key}Do`) }}
          </Button>
          <RouterLink
            v-else
            :to="{ name: 'automations', query: { new: 'salary' } }"
            :class="cn(buttonVariants({ size: 'sm' }), 'max-md:h-10')"
          >
            {{ t('home.steps.salaryDo') }}
          </RouterLink>
        </template>
      </div>
    </div>
    <StoreCreateDialog v-model:open="createOpen" />
  </Card>
</template>
