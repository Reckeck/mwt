<script setup lang="ts">
import { computed } from 'vue'
import { useI18n } from 'vue-i18n'

/**
 * `Where the data comes from`: who gets to see the public addresses and the device's IP, said plainly and
 * built from the addresses in use, so a node the user set appears here by its host.
 */
const props = defineProps<{ networks: NetworkView[] }>()
const { t } = useI18n()

const unique = (hosts: string[]) => [...new Set(hosts.filter(Boolean))].join(', ')
const nodes = computed(() => unique(props.networks.map((network) => network.rpc.host)))
const scouts = computed(() => unique(props.networks.map((network) => network.blockscout.host)))
</script>

<template>
  <div class="flex flex-col gap-1 rounded-lg bg-muted p-4 text-sm" data-tour="crypto-network-sources">
    <h3 class="font-medium text-muted-foreground">{{ t('cryptoTrack.network.sourcesTitle') }}</h3>
    <p>{{ t('cryptoTrack.network.sourcesWho') }}</p>
    <p><span class="font-medium">{{ t('cryptoTrack.network.sourcesNodes') }}</span> {{ nodes }}</p>
    <p v-if="scouts"><span class="font-medium">Blockscout:</span> {{ scouts }}</p>
    <p class="text-muted-foreground">{{ t('cryptoTrack.network.sourcesRest') }}</p>
  </div>
</template>
