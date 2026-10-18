<script setup lang="ts">
import { computed, ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { useIntervalFn } from '@vueuse/core'
import { Copy, ShieldAlert, ShieldCheck, WifiOff } from 'lucide-vue-next'

import { useSyncActions, useSyncStatus } from '@/features/vault-sync'
import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { Button } from '@/shared/ui/button'

/** Why devices do not see each other: the port, the address, who is in sight, the firewall. */
const { t } = useI18n()
const { status } = useSyncStatus()
const { openFirewall, copyCommand } = useSyncActions()

/** How long a device may look before the «nobody here» hint shows. */
const LONELY_AFTER_MS = 30_000

const network = computed(() => status.data.value?.network)
const now = ref(Date.now())
useIntervalFn(() => {
  now.value = Date.now()
}, 5000)

const line = computed(() => {
  const view = network.value
  if (!view?.port) return ''
  const parts = [t('sync.network.port', { port: view.port })]
  if (view.address) parts.push(view.address)
  parts.push(t('sync.network.seen', { count: view.seen }))
  return parts.join(' · ')
})
const lonely = computed(() => {
  const view = network.value
  if (!view?.since_ms || view.seen > 0) return false
  return now.value - view.since_ms > LONELY_AFTER_MS
})
const fix = computed(() => network.value?.firewall_fix ?? 'none')
/** The firewall block: after another device failed to get in, or while nobody shows up. */
const showFirewall = computed(() => fix.value !== 'none' && (network.value?.inbound_blocked || lonely.value))
const lastError = computed(() => {
  const code = network.value?.last_error
  return code ? t(`errors.${code}`) : ''
})
</script>

<template>
  <div v-if="line" class="grid gap-3">
    <div class="grid gap-0.5 rounded-md border border-border px-3 py-2">
      <span class="text-xs font-medium text-muted-foreground">{{ t('sync.network.title') }}</span>
      <span class="font-mono text-xs tabular-nums">{{ line }}</span>
      <span v-if="lastError" class="text-xs text-destructive">{{ t('sync.network.lastError', { error: lastError }) }}</span>
    </div>

    <Alert v-if="lonely">
      <WifiOff />
      <AlertTitle>{{ t('sync.network.lonelyTitle') }}</AlertTitle>
      <AlertDescription>{{ t('sync.network.lonelyHint') }}</AlertDescription>
    </Alert>

    <Alert v-if="showFirewall" :variant="network?.inbound_blocked ? 'destructive' : 'default'">
      <ShieldAlert />
      <AlertTitle>
        {{ network?.inbound_blocked ? t('sync.firewall.blocked') : t('sync.firewall.maybe') }}
      </AlertTitle>
      <AlertDescription class="grid gap-2">
        <span>{{ fix === 'auto' ? t('sync.firewall.autoHint') : t('sync.firewall.manualHint') }}</span>
        <Button
          v-if="fix === 'auto'"
          size="sm"
          class="justify-self-start"
          :disabled="openFirewall.isPending.value"
          @click="openFirewall.mutate()"
        >
          <ShieldCheck class="size-3.5" />
          {{ t('sync.firewall.open') }}
        </Button>
        <div v-for="command in network?.firewall_commands ?? []" :key="command" class="flex min-w-0 items-start gap-1">
          <code class="min-w-0 flex-1 break-all rounded bg-muted px-2 py-1 font-mono text-[11px] text-foreground">{{ command }}</code>
          <Button
            variant="ghost"
            size="icon-xs"
            class="shrink-0"
            :aria-label="t('sync.firewall.copy')"
            :title="t('sync.firewall.copy')"
            @click="copyCommand.mutate(command)"
          >
            <Copy class="size-3.5" />
          </Button>
        </div>
      </AlertDescription>
    </Alert>
  </div>
</template>
