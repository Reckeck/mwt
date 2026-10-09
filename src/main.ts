import { createPinia } from 'pinia'
import { createApp } from 'vue'

import App from '@/App.vue'
import { i18n } from '@/shared/config/i18n'
import { VueQueryPlugin, queryClient } from '@/app/providers/query'
import { router } from '@/app/providers/router'
import '@/app/styles.css'

const app = createApp(App)

app.use(router)
app.use(createPinia())
app.use(VueQueryPlugin, { queryClient })
app.use(i18n)

app.mount('#app')
