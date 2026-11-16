<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

import { Button } from '@/shared/ui/button'
import { SettingsSection } from '@/shared/ui/settings-section'

import type { NetworkTestOutcomeNote } from '../model/use-crypto-network'

/**
 * `Networks and nodes` (Settings · Crypto · Networks): one row per network with the address in use, the last
 * check and the two actions. The row's text is the host only: a path may carry a provider key.
 */
const props = defineProps<{
  networks: NetworkView[]
  selected: string
  /** The last test of each network's node, kept while the screen is open. */
  results: Record<string, NetworkTestOutcomeNote['result']>
  testing: string | null
}>()
const emit = defineEmits<{ select: [chain: string]; test: [network: NetworkView] }>()

const { t, n } = useI18n()

interface Row {
  network: NetworkView
  origin: string
  check: { text: string; ok: boolean } | null
}

function lastCheck(chain: string): Row['check'] {
  const result = props.results[chain]
  if (!result) return null
  if (result.outcome !== 'ok') return { text: t(`cryptoTrack.network.outcome.${result.outcome}`), ok: false }
  const ms = t('cryptoTrack.network.ms', { ms: n(Number(result.latency_ms)) })
  const block = result.block === undefined ? '' : ` · ${t('cryptoTrack.network.block', { block: n(Number(result.block)) })}`
  return { text: ms + block, ok: true }
}

const rows = computed<Row[]>(() =>
  props.networks.map((network) => ({
    network,
    origin: network.rpc.custom ? t('cryptoTrack.network.own') : t('cryptoTrack.network.preset'),
    check: lastCheck(network.chain),
  })),
)
</script>

<template>
  <SettingsSection :title="t('cryptoTrack.network.listTitle')" :description="t('cryptoTrack.network.listHint')">
    <ul class="flex flex-col gap-1" data-tour="crypto-network-list">
      <li
        v-for="row in rows"
        :key="row.network.chain"
        class="flex items-start justify-between gap-3 rounded-md p-2"
        :class="row.network.chain === props.selected ? 'bg-muted' : ''"
      >
        <div class="flex min-w-0 flex-col">
          <span class="font-medium">{{ row.network.label }}</span>
          <span class="break-words text-sm text-muted-foreground">
            {{ row.origin }} · {{ row.network.rpc.host }}<template v-if="row.check"> ·
              <span :class="row.check.ok ? '' : 'text-destructive'">{{ row.check.text }}</span></template>
          </span>
        </div>
        <div class="flex shrink-0 items-center gap-1">
          <Button
            variant="ghost"
            size="sm"
            :aria-current="row.network.chain === props.selected ? 'true' : undefined"
            @click="emit('select', row.network.chain)"
          >{{ t('cryptoTrack.network.configure') }}</Button>
          <Button variant="link" size="sm" :disabled="props.testing === row.network.chain" @click="emit('test', row.network)">
            {{ t('cryptoTrack.network.check') }}
          </Button>
        </div>
      </li>
    </ul>
  </SettingsSection>
</template>
