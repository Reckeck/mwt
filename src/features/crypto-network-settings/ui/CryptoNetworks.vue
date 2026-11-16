<script setup lang="ts">
import { computed, ref, useTemplateRef, watch } from 'vue'

import { SectionState } from '@/shared/ui/section-state'

import { type NetworkTestOutcomeNote, useNetworkSettings, useSaveNetwork, useTestNetwork } from '../model/use-crypto-network'
import DataSources from './DataSources.vue'
import NetworkEditor from './NetworkEditor.vue'
import NetworkList from './NetworkList.vue'

/** Settings · Crypto · Networks: the list, the editor of the chosen network and who sees the addresses. */
const query = useNetworkSettings()
const save = useSaveNetwork()
const results = ref<Record<string, NetworkTestOutcomeNote['result']>>({})
const test = useTestNetwork((note) => {
  results.value = { ...results.value, [note.chain]: note.result }
})
const editor = useTemplateRef<InstanceType<typeof NetworkEditor>>('editor')

const networks = computed(() => query.data.value?.networks ?? [])
const chosen = ref('')
watch(networks, (list) => { if (!list.some((network) => network.chain === chosen.value)) chosen.value = list[0]?.chain ?? '' }, { immediate: true })
const current = computed(() => networks.value.find((network) => network.chain === chosen.value))
const testing = computed(() => (test.isPending.value ? (test.variables.value?.input.chain ?? null) : null))

function onSave(input: SetNetworkSettingsInput) {
  save.mutate(input, { onError: (err) => editor.value?.showRefusal(err) })
}

function runTest(network: NetworkView) {
  chosen.value = network.chain
  test.mutate({ input: { chain: network.chain, kind: 'rpc' }, label: network.label })
}
</script>

<template>
  <SectionState :loading="query.isPending.value" :error="query.error.value" @retry="query.refetch()">
    <div class="flex flex-col gap-4">
      <NetworkList :networks="networks" :selected="chosen" :results="results" :testing="testing" @select="chosen = $event" @test="runTest" />
      <NetworkEditor
        v-if="current"
        ref="editor"
        :network="current"
        :editable="query.data.value?.editable ?? false"
        :pending="save.isPending.value"
        :testing="test.isPending.value"
        @save="onSave"
        @test="(input) => test.mutate({ input, label: current?.label ?? '' })"
      />
      <DataSources :networks="networks" />
    </div>
  </SectionState>
</template>
