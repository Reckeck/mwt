<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Badge } from '@/shared/ui/badge'
import { Card } from '@/shared/ui/card'

import type { ContactCardProps } from './contact-card.types'

const props = defineProps<ContactCardProps>()

/** Actions (edit, archive, write a transfer) come from the page. */
defineSlots<{ actions(): unknown }>()

const { t } = useI18n()

/** Accounts under their group label; ungrouped first. */
const groups = computed(() => {
  const out = new Map<string, ContactAccount[]>()
  for (const account of props.contact.accounts) {
    const key = account.group ?? ''
    out.set(key, [...(out.get(key) ?? []), account])
  }
  return [...out.entries()]
})
</script>

<template>
  <Card :class="props.contact.archived && 'opacity-60'">
    <template #header>
      <div class="flex flex-wrap items-center justify-between gap-2">
        <div class="flex min-w-0 items-center gap-2">
          <p class="truncate font-medium">{{ props.contact.name }}</p>
          <Badge variant="secondary">{{ t(`contactKind.${props.contact.kind}`) }}</Badge>
          <Badge v-if="props.contact.archived" variant="outline">{{ t('stores.archived') }}</Badge>
        </div>
        <div class="flex items-center gap-1">
          <slot name="actions" />
        </div>
      </div>
    </template>

    <div class="grid gap-4">
      <p v-if="props.contact.note" class="text-sm text-muted-foreground">{{ props.contact.note }}</p>

      <!-- Debts: what this person owes me and what I owe them. -->
      <div v-if="props.balances.length" class="grid gap-1">
        <p class="text-xs text-muted-foreground">{{ t('debt.balance') }}</p>
        <div v-for="row in props.balances" :key="row.asset.symbol" class="flex flex-wrap gap-x-4 text-sm tabular-nums">
          <span v-if="Number(row.receivable)" class="text-emerald-600 dark:text-emerald-400">
            {{ t('debt.owesMe') }}: {{ row.receivable }} {{ row.asset.symbol }}
          </span>
          <span v-if="Number(row.payable)" class="text-destructive">
            {{ t('debt.iOwe') }}: {{ row.payable }} {{ row.asset.symbol }}
          </span>
        </div>
      </div>

      <div v-if="groups.length" class="grid gap-3">
        <div v-for="[group, accounts] in groups" :key="group" class="grid gap-1">
          <p v-if="group" class="text-xs text-muted-foreground">{{ group }}</p>
          <div
            v-for="account in accounts"
            :key="account.id"
            class="flex flex-wrap items-center gap-x-2 text-sm"
            :class="!account.active && 'opacity-60'"
          >
            <span class="font-medium">{{ account.label || t(`detailKind.${account.kind}`) }}</span>
            <span class="font-mono text-xs break-all text-muted-foreground">{{ account.value }}</span>
            <Badge v-if="account.currency" variant="outline">{{ account.currency.toUpperCase() }}</Badge>
            <Badge v-if="!account.active" variant="outline">{{ t('contacts.inactive') }}</Badge>
          </div>
        </div>
      </div>
      <p v-else class="text-sm text-muted-foreground">{{ t('contacts.noAccounts') }}</p>

      <div v-if="props.turnover?.rows.length" class="grid gap-1">
        <p class="text-xs text-muted-foreground">
          {{ t('contacts.turnover') }} · {{ t('contacts.txCount', { count: props.turnover.tx_count }) }}
        </p>
        <div
          v-for="row in props.turnover.rows"
          :key="row.asset.symbol"
          class="flex flex-wrap gap-x-4 text-sm tabular-nums"
        >
          <span class="w-16 font-medium">{{ row.asset.symbol }}</span>
          <span class="text-destructive">{{ t('contacts.sent') }} {{ row.sent }}</span>
          <span class="text-emerald-600 dark:text-emerald-400">{{ t('contacts.received') }} {{ row.received }}</span>
        </div>
      </div>
    </div>
  </Card>
</template>
