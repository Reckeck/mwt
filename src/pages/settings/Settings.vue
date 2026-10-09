<script setup lang="ts">
import { VaultDevTools } from '@/widgets/vault-dev-tools'
import { VaultJsonEditor } from '@/widgets/vault-json-editor'
import { VaultSettings } from '@/widgets/vault-settings'

const isDev = import.meta.env.DEV
</script>

<template>
  <div class="mx-auto grid max-w-3xl gap-4">
    <VaultSettings />
    <VaultDevTools v-if="isDev">
      <template #viewer>
        <VaultJsonEditor />
      </template>
    </VaultDevTools>
  </div>
</template>
