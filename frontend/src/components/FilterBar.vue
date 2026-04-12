<script lang="ts" setup>
import { reactive, watch, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { toIsoDate } from '@/utils/dateFormat'

const emit = defineEmits<{
  (_e: 'filters-changed', _payload: { q: string; status: string; tags: string[] }): void
  (
    _e: 'filters-submitted',
    _payload: {
      q?: string
      status?: string
      tags?: string[]
      file?: string
      anchor_date?: string | null
      last_n_days?: number | null
    }
  ): void
}>()

const route = useRoute()
const router = useRouter()

// default filter shape
const filters = reactive({
  q: '',
  status: '', // e.g. 'open' | 'closed' | ''
  tags: [] as string[], // multi-select
  file: '',
  anchor_date: '' as string, // iso date string or empty
  last_n_days: null as number | null
})

// initialize from URL query
onMounted(() => {
  const q = route.query.q
  const status = route.query.status
  const tags = route.query.tags
  const file = route.query.file
  const from_date = route.query.from_date
  const last_n_days = route.query.last_n_days

  if (typeof q === 'string') filters.q = q
  if (typeof status === 'string') filters.status = status
  if (typeof tags === 'string') filters.tags = tags ? tags.split(',') : []
  if (Array.isArray(tags)) filters.tags = tags as string[]
  if (typeof file === 'string') filters.file = file
  if (typeof from_date === 'string') filters.anchor_date = toIsoDate(from_date) || ''
  if (typeof last_n_days === 'string') {
    const n = parseInt(last_n_days, 10)
    filters.last_n_days = isNaN(n) ? null : n
  }
})

// debounce helper
let timer: ReturnType<typeof setTimeout> | null = null
const DEBOUNCE_MS = 300

watch(
  () => ({ q: filters.q, status: filters.status, tags: [...filters.tags] }),
  () => {
    if (timer) clearTimeout(timer)
    timer = setTimeout(() => applyFilters(), DEBOUNCE_MS)
  },
  { deep: true }
)

function applyFilters() {
  // build query object, omit empty values
  const query: Record<string, string | string[]> = {}
  if (filters.q) query.q = filters.q
  if (filters.status) query.status = filters.status
  if (filters.tags && filters.tags.length) query.tags = filters.tags
  if (filters.file) query.file = filters.file
  if (filters.anchor_date) query.anchor_date = filters.anchor_date
  if (filters.last_n_days != null) query.last_n_days = String(filters.last_n_days)

  router.replace({ query }).catch(() => {})
  emit('filters-changed', {
    q: filters.q,
    status: filters.status,
    tags: [...filters.tags]
  })
}

function submitFilters() {
  const fromIsoDate = toIsoDate(filters.anchor_date)

  // emit richer payload for backend fetch
  emit('filters-submitted', {
    q: filters.q || undefined,
    status: filters.status || undefined,
    tags: filters.tags.length ? [...filters.tags] : undefined,
    file: filters.file || undefined,
    anchor_date: fromIsoDate || null,
    last_n_days: filters.last_n_days ?? null
  })
  // also update URL and live changed event
  applyFilters()
}

function clearFilters() {
  filters.status = ''
  filters.tags = []
  filters.file = ''
  filters.anchor_date = ''
  filters.last_n_days = null
  applyFilters()
}

</script>

<template>
  <b-card class="filter-bar">
    <b-card-body>
      <b-row class="align-items-center g-2">
        <b-col cols="2">
          <b-form-input v-model="filters.file" aria-label="File" placeholder="File path..." />
        </b-col>

        <b-col class="d-flex gap-1" cols="4">
          <b-form-input v-model="filters.anchor_date" aria-label="Anchor date" type="date" />
          <b-form-input v-model.number="filters.last_n_days" min="0" placeholder="60" type="number" />
        </b-col>

        <b-col class="mt-2" cols="12">
          <b-button variant="primary" @click="submitFilters">Apply</b-button>
          <b-button class="ms-2" variant="outline-secondary" @click="clearFilters">Clear</b-button>
        </b-col>
      </b-row>
    </b-card-body>
  </b-card>
</template>

<style scoped>
.filter-bar {
  padding: 0.5rem;
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.tags {
  display: flex;
  gap: 0.25rem;
}
</style>
