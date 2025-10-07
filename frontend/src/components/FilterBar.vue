<script lang="ts" setup>
import {reactive, watch, onMounted} from 'vue'
import {useRoute, useRouter} from 'vue-router'


const emit: (event: 'filters-changed', payload: {
  q: string;
  status: string;
  tags: string[]
}) => void = defineEmits()

const route = useRoute()
const router = useRouter()

// default filter shape
const filters = reactive({
  q: '',
  status: '',        // e.g. 'open' | 'closed' | ''
  tags: [] as string[] // multi-select
})

// sample tag options (adjust to your app)
const tagOptions = ['frontend', 'backend', 'bug', 'enhancement']

// initialize from URL query
onMounted(() => {
  const q = route.query.q
  const status = route.query.status
  const tags = route.query.tags

  if (typeof q === 'string') filters.q = q
  if (typeof status === 'string') filters.status = status
  if (typeof tags === 'string') filters.tags = tags ? tags.split(',') : []
  if (Array.isArray(tags)) filters.tags = tags as string[]
})

// debounce helper
let timer: ReturnType<typeof setTimeout> | null = null
const DEBOUNCE_MS = 300

watch(
    () => ({q: filters.q, status: filters.status, tags: [...filters.tags]}),
    () => {
      if (timer) clearTimeout(timer)
      timer = setTimeout(() => applyFilters(), DEBOUNCE_MS)
    },
    {deep: true}
)

function applyFilters() {
  // build query object, omit empty values
  const query: Record<string, string | string[]> = {}
  if (filters.q) query.q = filters.q
  if (filters.status) query.status = filters.status
  if (filters.tags && filters.tags.length) query.tags = filters.tags

  router.replace({query}).catch(() => {
  })
  emit('filters-changed', {
    q: filters.q,
    status: filters.status,
    tags: [...filters.tags]
  })
}

function clearFilters() {
  filters.q = ''
  filters.status = ''
  filters.tags = []
  applyFilters()
}

function toggleTag(tag: string) {
  const idx = filters.tags.indexOf(tag)
  if (idx === -1) filters.tags.push(tag)
  else filters.tags.splice(idx, 1)
}
</script>

<template>
  <b-card class="filter-bar">
    <b-card-body>
      <b-row class="align-items-center g-2">
        <b-col>
          <b-form-input
              v-model="filters.q"
              aria-label="Search"
              placeholder="Search..."
              type="search"
          />
        </b-col>
        <b-col>
          <b-form-select v-model="filters.status" aria-label="Status">
            <option value="">Any status</option>
            <option value="open">Open</option>
            <option value="closed">Closed</option>
          </b-form-select>
        </b-col>
        <b-col>
          <b-form-checkbox-group
              v-model="filters.tags"
              :options="tagOptions"
              class="tags"
              name="tags"
              stacked
          />
        </b-col>
        <b-col>
          <b-button variant="primary" @click="applyFilters">Apply</b-button>
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

.filter-row {
  display: flex;
  gap: 0.5rem;
  align-items: center;
}

.tags {
  display: flex;
  gap: 0.25rem;
}

.tag-item {
  display: flex;
  gap: 0.25rem;
  align-items: center;
}
</style>
