<script setup lang="ts">
import { computed, ref, watch } from 'vue'
import { useI18n } from 'vue-i18n'
import { Plus, X } from 'lucide-vue-next'
import { useQueryClient } from '@tanstack/vue-query'

import { CurrencyPicker, useFavoriteCurrencies } from '@/entities/currency'
import { Badge } from '@/shared/ui/badge'
import { Button } from '@/shared/ui/button'
import { call } from '@/shared/api/ipc'
import { notify } from '@/shared/lib/notify'
import { useSettingsStore } from '@/shared/store/settings'
import { useVaultStore } from '@/shared/store/vault'

/**
 * «Избранные валюты» (Currency-1 · 1): a chip per favourite with × (undone
 * from the toast) and «+ Валюта» that opens the currency list. The order is
 * the order they were added in; it ranks the conversions under an amount.
 */
const { t } = useI18n()
const settings = useSettingsStore()
const vault = useVaultStore()
const queryClient = useQueryClient()
const favorites = useFavoriteCurrencies()

/** The base currency always comes first anyway: no chip for it. */
const chips = computed(() => favorites.codes.value.filter((code) => code !== settings.baseCurrency))

const picked = ref('')
watch(picked, (code) => {
  if (!code) return
  picked.value = ''
  void add(code.toUpperCase())
})

async function add(code: string) {
  if (code.toLowerCase() === settings.baseCurrency || favorites.isFavorite(code)) {
    notify.error(null, t('validation.duplicateCurrency'))
    return
  }
  try {
    await favorites.add(code)
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    await settings.loadApp().catch(() => {})
    return
  }
  // The backend drops a code it cannot use; «Сохранено» would then be a lie.
  if (!favorites.isFavorite(code)) {
    notify.error(null, t('settings.favoriteRejected', { code }))
    return
  }
  notify.saved()
  if (settings.cryptoCurrencies.includes(code.toLowerCase())) void priceNewCoin()
}

/** A new crypto favourite has no cached price yet: fetch one quietly, so its rate shows at once. */
async function priceNewCoin() {
  if (vault.locked) return
  try {
    await call<RefreshResult>('refresh_all')
    await queryClient.invalidateQueries({ queryKey: ['fx-table'] })
  } catch {
    /* offline: the coin gets its rate with the next scheduled refresh */
  }
}

async function saveList(list: string[]) {
  try {
    await favorites.save(list)
  } catch (err) {
    notify.error(err, 'settings.saveFailed')
    await settings.loadApp().catch(() => {})
  }
}

function remove(code: string) {
  const before = [...favorites.codes.value]
  void saveList(before.filter((entry) => entry !== code))
  notify.success(t('settings.favoriteRemoved', { code: code.toUpperCase() }), {
    action: { label: t('actions.undo'), onClick: () => void saveList(before) },
  })
}
</script>

<template>
  <div class="grid gap-1.5" data-currency-favorites>
    <span class="text-sm font-medium">{{ t('settings.favoriteCurrencies') }}</span>
    <div class="flex flex-wrap gap-1.5">
      <Badge v-for="code in chips" :key="code" variant="secondary" class="h-6 gap-1 pe-0.5 font-normal uppercase">
        {{ code }}
        <Button
          variant="ghost"
          size="icon-xs"
          class="text-muted-foreground hover:text-destructive"
          :aria-label="t('settings.favoriteCurrencyRemove', { code: code.toUpperCase() })"
          @click="remove(code)"
        >
          <X class="size-3" />
        </Button>
      </Badge>
      <CurrencyPicker v-model="picked">
        <template #trigger>
          <Button variant="outline" size="sm" class="h-6 touch-hit rounded-md px-2 text-xs font-medium text-primary" data-currency-add>
            <Plus class="size-3" />{{ t('currencySettings.addFavoriteChip') }}
          </Button>
        </template>
      </CurrencyPicker>
    </div>
    <p class="text-xs text-muted-foreground">{{ t('currencySettings.favoritesHint') }}</p>
  </div>
</template>
