<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useRoute } from 'vue-router'
import { Settings } from 'lucide-vue-next'

import { CryptoAddressCard, useCryptoAccounts, useCryptoSyncStatus } from '@/entities/crypto-account'
import {
  type QuarantineRow,
  QuarantineBlock,
  quarantineRows,
  TokenReviewDialog,
  useCryptoTokens,
  useTokenReview,
} from '@/entities/crypto-token'
import { settingsQuery } from '@/shared/lib/settings-link'
import { buttonVariants } from '@/shared/ui/button'

/**
 * The followed addresses under the positions: each one's card with the entry to the network settings (Settings,
 * Crypto), and the tokens that wait in its quarantine, recognised through a review or hidden. Nothing here adds
 * or removes an address: a connected address is read, never typed in or edited by hand.
 */
const { t } = useI18n()
const route = useRoute()
const accounts = useCryptoAccounts()
const tokens = useCryptoTokens()
const sync = useCryptoSyncStatus()
const { reject } = useTokenReview()

const reviewing = ref<QuarantineRow | null>(null)
const reviewOpen = ref(false)
const settingsTo = computed(() => ({ query: settingsQuery(route.query, 'crypto', 'crypto-networks') }))

const live = computed(() => (accounts.data.value ?? []).filter((account) => !account.archived))
const readAt = (id: string) => sync.data.value?.accounts?.find((entry) => entry.account_id === id)?.last_ok_at ?? null
const rowsOf = (account: CryptoAccountView) => quarantineRows(account.quarantine, tokens.data.value ?? [])

function review(row: QuarantineRow) {
  reviewing.value = row
  reviewOpen.value = true
}
</script>

<template>
  <section v-if="live.length > 0" class="grid gap-4" data-positions-addresses>
    <div class="flex flex-wrap items-center justify-between gap-3">
      <h2 class="text-base font-semibold">{{ t('cryptoTrack.addresses.title') }}</h2>
      <RouterLink :to="settingsTo" :class="buttonVariants({ variant: 'outline', size: 'sm' })" data-network-settings>
        <Settings />{{ t('cryptoTrack.addresses.settings') }}
      </RouterLink>
    </div>
    <div v-for="account in live" :key="account.id" class="grid gap-3" :data-address-account="account.id">
      <CryptoAddressCard :name="account.name" :chain="account.chain" :address="account.address" :source="account.source" :read-at="readAt(account.id)" />
      <QuarantineBlock
        v-if="rowsOf(account).length > 0"
        :rows="rowsOf(account)"
        :busy="reject.isPending.value"
        @recognize="review"
        @hide="reject.mutate($event)"
      />
    </div>
    <TokenReviewDialog v-model:open="reviewOpen" :row="reviewing" />
  </section>
</template>
