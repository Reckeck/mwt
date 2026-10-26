<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'

import { call } from '@/shared/api/ipc'
import { useStoreTree } from '@/shared/api/queries'
import { flattenStoreTree } from '@/shared/lib/store-tree'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'

/**
 * Full-New-Fixes 2: Settings › Storage in the sandbox. The vault lives in memory and is
 * erased on close; the file actions stay in view but off, and the demo's contents are counted.
 */
const { t } = useI18n()
const tree = useStoreTree()
const budgets = useQuery({ queryKey: ['budgets'], queryFn: () => call<Budget[]>('list_budgets') })
const goals = useQuery({ queryKey: ['goals'], queryFn: () => call<Goal[]>('list_goals') })
const workflows = useQuery({ queryKey: ['workflows', 'count'], queryFn: () => call<Workflow[]>('list_workflows') })

const nodes = computed(() => flattenStoreTree(tree.data.value ?? []))
const counts = computed(() => [
  { key: 'wallets', value: nodes.value.length },
  { key: 'operations', value: nodes.value.reduce((sum, node) => sum + node.own_tx_count, 0) },
  { key: 'limits', value: budgets.data.value?.length ?? 0 },
  { key: 'goals', value: goals.data.value?.length ?? 0 },
  { key: 'automations', value: workflows.data.value?.length ?? 0 },
])
const ACTIONS = ['setPassword', 'createCopy', 'backupNow'] as const
</script>

<template>
  <div class="grid grid-cols-1 gap-4" data-vault-sandbox>
    <SettingsSection :title="t('vaultFile.title')">
      <template #actions><Badge variant="warning" class="font-normal">{{ t('sandbox.name') }}</Badge></template>
      <dl class="grid grid-cols-[auto_1fr] gap-x-6 gap-y-2 text-sm">
        <dt class="text-muted-foreground">{{ t('sandbox.whereIs') }}</dt>
        <dd>{{ t('sandbox.inMemory') }}</dd>
        <dt class="text-muted-foreground">{{ t('sandbox.password') }}</dt>
        <dd>{{ t('sandbox.passwordNone') }}</dd>
      </dl>
      <p class="mt-2 text-sm text-muted-foreground">{{ t('sandbox.erasedOnClose') }}</p>
      <div class="mt-3 flex flex-wrap gap-2">
        <Button v-for="action in ACTIONS" :key="action" variant="outline" size="sm" class="max-md:h-10" disabled>
          {{ t(`sandbox.action.${action}`) }}
        </Button>
      </div>
      <p class="mt-2 text-xs text-muted-foreground">{{ t('sandbox.fileActionsOff') }}</p>
    </SettingsSection>
    <SettingsSection :title="t('sandbox.contents')">
      <dl class="grid grid-cols-[1fr_auto] gap-x-6 gap-y-1.5 text-sm" data-sandbox-counts>
        <template v-for="entry in counts" :key="entry.key">
          <dt class="text-muted-foreground">{{ t(`sandbox.count.${entry.key}`) }}</dt>
          <dd class="text-right tabular-nums">{{ entry.value }}</dd>
        </template>
      </dl>
    </SettingsSection>
  </div>
</template>
