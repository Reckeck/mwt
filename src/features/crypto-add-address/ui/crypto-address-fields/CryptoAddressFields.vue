<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { ShieldAlert } from 'lucide-vue-next'

import { Alert, AlertDescription, AlertTitle } from '@/shared/ui/alert'
import { FormField } from '@/shared/ui/form-field'
import { Input } from '@/shared/ui/input'
import { Select, SelectItem } from '@/shared/ui/select'

/**
 * Step 2 (mockups 09 and 11): the network, the public address and a name. The address is a plain
 * `Input` whose changes go through `setAddress` (the secret guard); a refused paste leaves the field
 * empty and says so in a box under it.
 */
const props = defineProps<{
  networks: { chain: Chain; label: string }[]
  address: string
  addressKey: number
  refused: boolean
  addressError: string | undefined
  nameError: string | undefined
}>()
const chain = defineModel<Chain>('chain', { required: true })
const name = defineModel<string>('name', { required: true })
const emit = defineEmits<{ address: [value: string] }>()
const { t } = useI18n()
</script>

<template>
  <div class="flex flex-col gap-4">
    <FormField for="crypto-network" :label="t('cryptoTrack.connect.network')" required :hint="t('cryptoTrack.connect.networkHint')">
      <Select id="crypto-network" v-model="chain">
        <SelectItem v-for="entry in props.networks" :key="entry.chain" :value="entry.chain">{{ entry.label }}</SelectItem>
      </Select>
    </FormField>

    <FormField for="crypto-address" :label="t('cryptoTrack.connect.address')" required :error="props.addressError" :hint="t('cryptoTrack.connect.addressHint')">
      <Input
        id="crypto-address"
        :key="props.addressKey"
        :model-value="props.address"
        class="font-mono"
        placeholder="0x…"
        autocomplete="off"
        autocapitalize="off"
        spellcheck="false"
        data-crypto-address-input
        @update:model-value="(value) => emit('address', String(value))"
      />
    </FormField>

    <Alert v-if="props.refused" variant="destructive" data-crypto-secret-refused>
      <ShieldAlert />
      <AlertTitle>{{ t('cryptoTrack.connect.refusedTitle') }}</AlertTitle>
      <AlertDescription>{{ t('cryptoTrack.connect.refusedBody') }}</AlertDescription>
    </Alert>

    <FormField for="crypto-name" :label="t('cryptoTrack.connect.name')" :error="props.nameError" :hint="t('cryptoTrack.connect.nameHint')">
      <Input id="crypto-name" v-model="name" />
    </FormField>
  </div>
</template>
