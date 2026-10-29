<script setup lang="ts">
import { useI18n } from 'vue-i18n'
import { FlaskConical } from 'lucide-vue-next'

import { Button } from '@/shared/ui/button'
import { Separator } from '@/shared/ui/separator'
import { useIsPhone } from '@/shared/lib/phone'

import { useSandboxOpen } from './model/use-sandbox-open'

/**
 * Full-New-Sandbox 1: under `No vault yet?` of the unlock card, below a rule.
 * Desktop: a flask, the link and a line of what it is; phone: a 48 px button
 * with a shorter line under it. The only way in: Help and Settings do not repeat it.
 */
const { t } = useI18n()
const isPhone = useIsPhone()
const open = useSandboxOpen()
</script>

<template>
  <div class="grid gap-3" data-tour="unlock-sandbox">
    <Separator />
    <Button
      v-if="isPhone"
      variant="outline"
      class="h-12 w-full gap-2"
      :loading="open.isPending.value"
      data-sandbox-open
      @click="open.mutate()"
    >
      <FlaskConical class="size-4" aria-hidden="true" />
      {{ t('sandbox.open') }}
    </Button>
    <div v-else class="flex items-start gap-3">
      <FlaskConical class="mt-2 size-4 shrink-0 text-muted-foreground" aria-hidden="true" />
      <div class="grid min-w-0 flex-1 justify-items-start gap-0.5">
        <Button
          variant="link"
          class="h-auto min-h-8 px-0"
          :loading="open.isPending.value"
          data-sandbox-open
          @click="open.mutate()"
        >
          {{ t('sandbox.open') }}
        </Button>
        <span class="text-xs text-muted-foreground">{{ t('sandbox.openHint') }}</span>
      </div>
    </div>
    <p v-if="isPhone" class="text-center text-xs text-muted-foreground">{{ t('sandbox.openHintShort') }}</p>
  </div>
</template>
