<script setup lang="ts">
import { computed } from 'vue'
import { RouterView, useRoute, useRouter } from 'vue-router'
import { useI18n } from 'vue-i18n'
import { ChevronLeft } from 'lucide-vue-next'

import { quickContacts } from '@/entities/contact'
import { Button } from '@/shared/ui/button'
import { RouteTabs } from '@/shared/ui/route-tabs'
import { useContacts } from '@/shared/api/queries'
import { useIsPhone } from '@/shared/lib/phone'

const { t } = useI18n()
const route = useRoute()
const router = useRouter()
const phone = useIsPhone()
const contacts = useContacts()
/** A contact on a phone is a screen of its own, with «‹» instead of the tabs. */
const inContact = computed(() => phone.value && route.name === 'transactions-contacts' && Boolean(route.query.contact))
/** So are the bonuses (Ops-2 · 12): `‹ Bonuses and cashback`, back to the list. */
const inBonuses = computed(() => phone.value && route.name === 'transactions-bonuses')
function back() {
  if (window.history.state?.back) router.back()
  else void router.push({ name: 'transactions' })
}

/** Each tab is a child route, so a tab has its own address. */
const tabs = computed(() => [
  { name: 'transactions', label: t('transactions.tab.list') },
  { name: 'transactions-calendar', label: t('transactions.tab.calendar') },
])
/** Used less often: under `More ▾`. */
const quickCount = computed(() => quickContacts(contacts.data.value ?? []).length)
const more = computed(() => [
  {
    name: 'transactions-contacts',
    label: quickCount.value ? `${t('transactions.tab.contacts')} · ${quickCount.value}` : t('transactions.tab.contacts'),
  },
  { name: 'transactions-bonuses', label: t('transactions.tab.bonuses') },
])
</script>

<template>
  <div class="grid grid-cols-1 gap-4 @container">
    <div v-if="inBonuses" class="-mx-2 flex items-center gap-1">
      <Button variant="ghost" size="icon" :aria-label="t('actions.back')" @click="back"><ChevronLeft class="size-5" /></Button>
      <h1 class="min-w-0 flex-1 truncate text-xl font-semibold">{{ t('transactions.tab.bonuses') }}</h1>
    </div>
    <div v-else-if="!inContact" class="flex min-w-0 flex-wrap items-center gap-2">
      <div class="w-fit max-w-full" data-tour="section-tabs"><RouteTabs :tabs="tabs" :more="more" :more-label="t('nav.more')" /></div>
      <!-- A tab's own controls on the tabs' row (the calendar's month), teleported in. -->
      <div id="tx-section-end" class="ml-auto flex min-w-0 items-center" />
    </div>
    <div class="min-w-0" data-tour="section-content"><RouterView /></div>
  </div>
</template>
