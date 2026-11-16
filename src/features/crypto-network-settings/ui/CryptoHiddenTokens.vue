<script setup lang="ts">
import { computed } from 'vue'
import { keepPreviousData, useMutation, useQuery, useQueryClient } from '@tanstack/vue-query'
import { useI18n } from 'vue-i18n'

import { useCryptoAccounts } from '@/entities/crypto-account'
import { CRYPTO_TOKENS_KEY } from '@/entities/crypto-token'
import { call } from '@/shared/api/ipc'
import { formatDate } from '@/shared/lib/format'
import { Button } from '@/shared/ui/button'
import { SectionState } from '@/shared/ui/section-state'
import { SettingsSection } from '@/shared/ui/settings-section'

import { useNetworkSettings } from '../model/use-crypto-network'

/**
 * Settings · Crypto · Tokens: the tokens the user hid, with a way back. Hiding is a mark in the vault, it changes
 * no amount and deletes no operation. The names are cleaned plain text (spec E2) and are never rendered as markup.
 */
const { t } = useI18n()
const queryClient = useQueryClient()
const tokens = useQuery({
  queryKey: [...CRYPTO_TOKENS_KEY, 'hidden'],
  queryFn: () => call<TokenView[]>('crypto_list_tokens'),
  placeholderData: keepPreviousData,
})
const networks = useNetworkSettings()
const accounts = useCryptoAccounts()

/** The followed addresses that have a wallet to open: the quarantine is decided there, on the assets tab. */
const withQuarantine = computed(() =>
  (accounts.data.value ?? [])
    .filter((account) => account.store_id && !account.archived)
    .map((account) => ({
      id: account.id,
      name: account.name,
      storeId: account.store_id as string,
      waiting: account.quarantine.length,
    })),
)

const hidden = computed(() => (tokens.data.value ?? []).filter((token) => token.status === 'rejected'))
const labelOf = (chain: string) => networks.data.value?.networks.find((entry) => entry.chain === chain)?.label ?? chain
const short = (contract: string) => (contract.length > 12 ? `${contract.slice(0, 6)}…${contract.slice(-3)}` : contract)

function change(verdict: 'crypto_confirm_token' | 'crypto_reject_token', token: TokenView) {
  return call(verdict, { input: { chain: token.chain, contract: token.contract } satisfies TokenRefInput })
}

const restore = useMutation({
  mutationFn: (token: TokenView) => change('crypto_confirm_token', token),
  onSuccess: () => void queryClient.invalidateQueries({ queryKey: CRYPTO_TOKENS_KEY }),
  meta: {
    success: (_data: unknown, token: unknown) => t('cryptoTrack.hiddenTokens.restored', { symbol: (token as TokenView).symbol }),
    error: 'cryptoTrack.hiddenTokens.failed',
    undo: async (_data: unknown, token: unknown) => {
      await change('crypto_reject_token', token as TokenView)
      void queryClient.invalidateQueries({ queryKey: CRYPTO_TOKENS_KEY })
    },
  },
})
</script>

<template>
  <div class="flex flex-col gap-4">
    <SettingsSection :title="t('cryptoTrack.hiddenTokens.title', { count: hidden.length })">
      <SectionState
        :loading="tokens.isPending.value"
        :error="tokens.error.value"
        :empty="hidden.length === 0"
        :empty-title="t('cryptoTrack.hiddenTokens.emptyTitle')"
        :empty-description="t('cryptoTrack.hiddenTokens.emptyBody')"
        compact
        @retry="tokens.refetch()"
      >
        <ul class="flex flex-col gap-1" data-tour="crypto-hidden-tokens">
          <li v-for="token in hidden" :key="`${token.chain}:${token.contract}`" class="flex items-start justify-between gap-3 rounded-md p-2">
            <div class="flex min-w-0 flex-col">
              <span class="break-words font-medium">{{ token.symbol }}</span>
              <span class="break-words text-sm text-muted-foreground">
                {{ labelOf(token.chain) }} · {{ short(token.contract) }} · {{ t('cryptoTrack.hiddenTokens.notCounted') }}
                <template v-if="token.decided_at">
                  · {{ t('cryptoTrack.hiddenTokens.hiddenOn', { date: formatDate(token.decided_at) }) }}
                </template>
              </span>
            </div>
            <Button variant="link" size="sm" :disabled="restore.isPending.value" @click="restore.mutate(token)">
              {{ t('cryptoTrack.hiddenTokens.restore') }}
            </Button>
          </li>
        </ul>
      </SectionState>
    </SettingsSection>
    <SettingsSection :title="t('cryptoTrack.hiddenTokens.quarantineTitle')">
      <p class="text-sm">{{ t('cryptoTrack.hiddenTokens.quarantineBody') }}</p>
      <!-- The decision itself belongs to the wallet, so this is the way there, one line per address. -->
      <ul v-if="withQuarantine.length" class="mt-3 flex flex-col gap-1">
        <li v-for="account in withQuarantine" :key="account.id" class="flex items-center justify-between gap-3">
          <span class="min-w-0 truncate text-sm">
            {{ account.name }}
            <span class="text-muted-foreground">
              · {{ account.waiting ? t('cryptoTrack.hiddenTokens.waiting', { count: account.waiting }) : t('cryptoTrack.hiddenTokens.waitingNone') }}
            </span>
          </span>
          <Button as-child variant="link" size="sm">
            <RouterLink :to="{ name: 'wallet', params: { id: account.storeId }, query: { tab: 'positions' } }">
              {{ t('cryptoTrack.hiddenTokens.openQuarantine') }}
            </RouterLink>
          </Button>
        </li>
      </ul>
      <p v-else class="mt-3 text-sm text-muted-foreground">{{ t('cryptoTrack.hiddenTokens.noAddress') }}</p>
    </SettingsSection>
    <div class="flex flex-col gap-1 rounded-lg bg-muted p-4 text-sm">
      <h3 class="font-medium text-muted-foreground">{{ t('cryptoTrack.hiddenTokens.noteTitle') }}</h3>
      <p>{{ t('cryptoTrack.hiddenTokens.noteBody') }}</p>
    </div>
  </div>
</template>
