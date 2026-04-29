import { createPinia } from 'pinia'

import { useUIStore } from './ui'
import { useSidebarStore } from './sidebar'

const pinia = createPinia()

export { useUIStore, useSidebarStore }

export default pinia
