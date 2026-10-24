<script setup lang="ts">
import { ref } from 'vue'
import { useI18n } from 'vue-i18n'
import { FlaskConical, LogOut, RotateCcw } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { useIsPhone } from '@/shared/lib/phone'

import SandboxResetDialog from './SandboxResetDialog.vue'
import { useSandboxBar } from './use-sandbox-bar'

/**
 * Full-New-Sandbox 2: a bar above the header while the sandbox is open, on every
 * screen. The warning token of the theme; the top inset is its own (the header
 * below it drops it). A phone says only «Песочница».
 */
const { t } = useI18n()
const isPhone = useIsPhone()
const { reset, exit } = useSandboxBar()
const resetOpen = ref(false)

function confirmReset() {
  reset.mutate(undefined, { onSuccess: () => (resetOpen.value = false) })
}
</script>

<template>
  <div
    class="box-content flex min-h-11 shrink-0 items-center gap-2.5 border-b border-warning/40 bg-warning/15 px-3 pt-safe md:min-h-10 md:px-6"
    role="status"
    data-sandbox-bar
  >
    <FlaskConical class="size-4 shrink-0 text-warning" aria-hidden="true" />
    <p class="min-w-0 flex-1 truncate text-sm">
      <b class="font-semibold">{{ t('sandbox.name') }}</b>
      <template v-if="!isPhone">{{ ' ' + t('sandbox.barRest') }}</template>
    </p>
    <Button variant="outline" size="sm" class="shrink-0 gap-1.5" data-sandbox-reset @click="resetOpen = true">
      <RotateCcw class="size-3.5" aria-hidden="true" />
      {{ t('sandbox.reset') }}
    </Button>
    <Button
      variant="outline"
      size="sm"
      class="shrink-0 gap-1.5"
      :loading="exit.isPending.value"
      data-sandbox-exit
      @click="exit.mutate()"
    >
      <LogOut class="size-3.5" aria-hidden="true" />
      {{ t('sandbox.exit') }}
    </Button>
    <SandboxResetDialog v-model:open="resetOpen" :loading="reset.isPending.value" @confirm="confirmReset" />
  </div>
</template>
