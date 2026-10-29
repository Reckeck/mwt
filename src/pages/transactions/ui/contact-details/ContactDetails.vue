<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useQuery } from '@tanstack/vue-query'
import { useRouter } from 'vue-router'
import { ArrowDownLeft, ArrowUpRight, ChevronDown, ChevronLeft, Pencil, ScrollText, Undo2, User } from 'lucide-vue-next'

import { ContactAccounts, ContactEditDialog } from '@/features/contact-edit'
import { DebtTermsPopover } from '@/features/debt-manage'
import { ContactSharedEvents } from '@/widgets/contact-shared-events'
import { ContactTurnoverTable, accountCounts, contactNets, isQuickContact } from '@/entities/contact'
import { Badge } from '@/shared/ui/badge'
import { Money, useConversions } from '@/entities/currency'
import { TransactionsList } from '@/entities/transaction'
import { ActionMenu } from '@/shared/ui/action-menu'
import { Button } from '@/shared/ui/button'
import { Card } from '@/shared/ui/card'
import { Collapsible, CollapsibleContent, CollapsibleTrigger } from '@/shared/ui/collapsible'
import type { UiAction } from '@/shared/ui/context-menu'
import { EmptyState } from '@/shared/ui/empty-state'
import { call } from '@/shared/api/ipc'
import { useSettingsStore } from '@/shared/store/settings'
import { emptyTxFilter } from '@/shared/lib/tx-filter'
import { openTxDetails } from '@/shared/lib/tx-details'
import { pluralForm } from '@/shared/lib/format'
import { cn } from '@/shared/lib/utils'

import type { ContactTransfer } from '../../model/contact-transfer.types'
import { ContactDebtPhrase } from '../contact-debt-phrase'

/**
 * One contact (Ops-1 · 6): who it is, the debt in one line with its term, the
 * requisites as records, and the operations with it. Debt operations and the
 * contact's own fields are in «⋯»; the turnover table folds away at the bottom.
 */
const props = defineProps<{
  contact: Contact
  balances: ContactBalance[]
  transferBlocked: string | false
  /** A screen of its own: «‹ name ⋯» on top, no card frame, the two buttons full width. */
  phone?: boolean
}>()
const emit = defineEmits<{ transfer: [transfer: ContactTransfer]; deleted: []; back: [] }>()

const { t, locale } = useI18n()
const plural = (key: string, count: number) => t(`${key}.${pluralForm(count, locale.value)}`, { count })
const router = useRouter()
const settings = useSettingsStore()
const conversions = useConversions()

const nets = computed(() => contactNets(props.balances, props.contact.id))
/** The whole debt in the base currency, when every asset of it has a rate. */
const netInBase = computed(() => {
  const parts = nets.value.map((row) => conversions.convertTo(row.net, row.symbol, settings.baseCurrency))
  return parts.length && parts.every((part) => part !== null) ? parts.reduce<number>((sum, part) => sum + (part ?? 0), 0) : null
})
const showBase = computed(() => nets.value.some((row) => row.symbol.toLowerCase() !== settings.baseCurrency.toLowerCase()))

/** «Вернули долг» when they owe, «Вернуть долг» when you do: the next thing a debt asks for. */
const settle = computed<DebtEffect | null>(() => {
  const first = nets.value[0]
  if (!first) return null
  return first.net > 0 ? 'collect' : 'repay'
})

/** «Человек · Семья · 3 реквизита». */
const subtitle = computed(() => {
  const { active } = accountCounts(props.contact.accounts)
  return [
    t(`contactKind.${props.contact.kind}`),
    props.contact.group.trim(),
    props.contact.accounts.length ? plural('contacts.accountCount', active) : '',
  ]
    .filter(Boolean)
    .join(' · ')
})

const history = useQuery({
  // no placeholder: another contact's operations
  queryKey: computed(() => ['transactions', 'contact', props.contact.id]),
  queryFn: () => call<Transaction[]>('list_transactions', { filter: emptyTxFilter({ contact_id: props.contact.id }) }),
})
const turnover = useQuery({
  // no placeholder: another contact's turnover
  queryKey: computed(() => ['contacts', 'turnover', props.contact.id]),
  queryFn: () => call<ContactTurnover>('get_contact_turnover', { input: { contact_id: props.contact.id, account_id: null } }),
})

const editOpen = ref(false)
const turnoverOpen = ref(false)

const transfer = (effect: DebtEffect | null, account: ContactAccount | null = null) =>
  emit('transfer', { effect, accountId: account?.id ?? null })

const menu = computed<UiAction[]>(() => {
  const debt = (effect: DebtEffect, icon: UiAction['icon']): UiAction => ({
    id: effect,
    label: t(`debt.action.${effect}`),
    icon,
    disabled: props.transferBlocked,
    run: () => transfer(effect),
  })
  return [
    debt('lend', ArrowUpRight),
    debt('borrow', ArrowDownLeft),
    debt('repay', Undo2),
    debt('collect', Undo2),
    {
      id: 'transactions',
      label: t('contacts.allTransactions'),
      icon: ScrollText,
      separated: true,
      run: () => void router.push({ name: 'transactions', query: { contact: props.contact.id } }),
    },
    { id: 'edit', label: t('contacts.edit'), icon: Pencil, run: () => (editOpen.value = true) },
  ]
})
</script>

<template>
  <component
    :is="props.phone ? 'div' : Card"
    :class="cn('@container min-w-0', props.contact.archived && 'opacity-70')"
    data-tour="contact-editor"
  >
    <div class="grid grid-cols-1 gap-5">
      <div v-if="props.phone" class="-mx-2 flex items-center gap-1">
        <Button variant="ghost" size="icon" :aria-label="t('actions.back')" @click="emit('back')">
          <ChevronLeft class="size-5" />
        </Button>
        <h1 class="min-w-0 flex-1 truncate text-xl font-semibold">{{ props.contact.name }}</h1>
        <ActionMenu :actions="menu" :label="t('actions.more')" :sheet="{ title: props.contact.name, description: subtitle }" />
      </div>
      <header class="flex flex-wrap items-center gap-3">
        <span class="flex size-11 shrink-0 items-center justify-center rounded-lg bg-secondary text-muted-foreground">
          <User class="size-5" />
        </span>
        <div class="min-w-0 flex-1">
          <h2 :class="cn('flex items-center gap-2 font-semibold', props.phone ? 'text-base' : 'text-xl')">
            <span class="truncate">{{ props.contact.name }}</span>
            <Badge v-if="isQuickContact(props.contact)" variant="outline" class="shrink-0 font-normal">{{ t('contacts.quick.badge') }}</Badge>
          </h2>
          <p v-if="!props.phone" class="truncate text-sm text-muted-foreground">{{ subtitle }}</p>
          <p v-else class="truncate text-sm">
            <ContactDebtPhrase :nets="nets" :empty="t('contacts.debt.none')" />
            <template v-if="nets.length">
              · <DebtTermsPopover :contact-id="props.contact.id" :asset="nets[0].symbol" />
            </template>
          </p>
        </div>
        <div class="flex w-full items-center gap-2 @lg:w-auto">
          <Button
            variant="outline"
            :class="cn('flex-1 @lg:flex-none', props.phone && 'h-12')"
            :disabled="Boolean(props.transferBlocked)"
            :title="props.transferBlocked || undefined"
            data-tour="contact-transactions"
            @click="transfer(null)"
          >
            {{ props.phone ? t('contacts.transferShort') : t('contacts.writeTransfer') }}
          </Button>
          <Button
            v-if="settle"
            :class="cn('flex-1 @lg:flex-none', props.phone && 'h-12')"
            :disabled="Boolean(props.transferBlocked)"
            :title="props.transferBlocked || undefined"
            @click="transfer(settle)"
          >
            {{ t(`contacts.settle.${settle}`) }}
          </Button>
          <ActionMenu
            v-if="!props.phone"
            :actions="menu"
            :label="t('actions.more')"
            :sheet="{ title: props.contact.name, description: subtitle }"
          />
        </div>
      </header>

      <div v-if="!props.phone" class="flex flex-wrap items-baseline gap-x-3 gap-y-1 text-sm" data-tour="contact-debts">
        <span class="text-muted-foreground">{{ t('contacts.debt.label') }}</span>
        <ContactDebtPhrase :nets="nets" :empty="t('contacts.debt.none')" strong />
        <span v-if="nets.length" class="flex flex-wrap items-baseline gap-x-1 text-xs text-muted-foreground">
          <template v-if="showBase && netInBase !== null">
            <Money :amount="Math.abs(netInBase)" :convert="false" />
            <span>·</span>
          </template>
          <DebtTermsPopover :contact-id="props.contact.id" :asset="nets[0].symbol" />
        </span>
        <span v-if="history.data.value" class="ml-auto text-xs text-muted-foreground">
          {{ plural('contacts.opsCount', history.data.value.length) }}
        </span>
      </div>

      <ContactAccounts
        :contact="props.contact"
        :transfer-disabled="props.transferBlocked"
        @transfer="(account) => transfer(null, account)"
      />

      <ContactSharedEvents :contact-id="props.contact.id" />

      <section class="grid grid-cols-1 gap-2">
        <h3 class="text-base font-semibold">{{ t('contacts.operations') }}</h3>
        <TransactionsList
          v-if="history.data.value?.length"
          :transactions="history.data.value"
          openable
          @open="openTxDetails"
        />
        <EmptyState v-else-if="history.isSuccess.value" compact :title="t('contacts.noHistory')" />
      </section>

      <Collapsible v-if="history.data.value?.length" v-model:open="turnoverOpen" class="grid gap-2">
        <CollapsibleTrigger as-child>
          <Button variant="ghost" size="sm" class="justify-start gap-1 px-0 text-muted-foreground hover:bg-transparent">
            <ChevronDown class="size-4 transition-transform" :class="!turnoverOpen && '-rotate-90'" />
            {{ t('contacts.turnover') }}
          </Button>
        </CollapsibleTrigger>
        <CollapsibleContent>
          <ContactTurnoverTable :turnover="turnover.data.value ?? null" />
        </CollapsibleContent>
      </Collapsible>
    </div>

    <ContactEditDialog v-model:open="editOpen" :contact="props.contact" @deleted="emit('deleted')" />
  </component>
</template>
