<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert, BFormSelect, BBadge } from 'bootstrap-vue-next'
import BarChartComponent from '@/components/Charts/BarChartComponent.vue'
import { getTopFiles, TopFilesResponse, DEFAULT_REPOSITORY } from '@/services/prApi'

const route = useRoute()
const router = useRouter()

const repository = ref<string>(DEFAULT_REPOSITORY)
const userName = ref<string>('')
const topN = ref<number>(100)
const lastNDays = ref<number>(30)
const depth = ref<number>(2)

const topFilesData = ref<TopFilesResponse[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

const depthOptions = [
  { value: 1, text: 'Depth 1  (e.g. compiler/)' },
  { value: 2, text: 'Depth 2  (e.g. compiler/rustc_ast/)' },
  { value: 3, text: 'Depth 3  (e.g. compiler/rustc_ast/src/)' }
]

onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.user) userName.value = String(route.query.user)
  if (route.query.top_n) topN.value = Number(route.query.top_n)
  if (route.query.last_n_days) lastNDays.value = Number(route.query.last_n_days)
  if (route.query.depth) depth.value = Number(route.query.depth)
  if (userName.value) fetchTopFiles()
})

watch([repository, userName, topN, lastNDays, depth], () => {
  router.replace({
    query: {
      ...(repository.value ? { repository: repository.value } : {}),
      ...(userName.value ? { user: userName.value } : {}),
      top_n: topN.value,
      last_n_days: lastNDays.value,
      depth: depth.value
    }
  })
}, { flush: 'post' })

/** Returns the directory prefix of a file path at the given depth level. */
function getPathPrefix(filePath: string, d: number): string {
  const parts = filePath.split('/')
  // If the file is shallower than the requested depth, use its parent dir
  const take = Math.min(d, parts.length - 1)
  return take > 0 ? parts.slice(0, take).join('/') : '(root)'
}

const groupedByPath = computed(() => {
  const counts = new Map<string, number>()
  for (const file of topFilesData.value) {
    const prefix = getPathPrefix(file.file_path, depth.value)
    counts.set(prefix, (counts.get(prefix) ?? 0) + 1)
  }
  return Array.from(counts.entries()).sort((a, b) => b[1] - a[1])
})

const PALETTE = [
  'rgba(54, 162, 235, 0.8)', 'rgba(255, 99, 132, 0.8)', 'rgba(255, 206, 86, 0.8)',
  'rgba(75, 192, 192, 0.8)', 'rgba(153, 102, 255, 0.8)', 'rgba(255, 159, 64, 0.8)',
  'rgba(201, 203, 207, 0.8)', 'rgba(83, 102, 255, 0.8)', 'rgba(255, 99, 255, 0.8)',
  'rgba(99, 255, 132, 0.8)'
]

const chartData = computed(() => ({
  labels: groupedByPath.value.map(([path]) => path),
  datasets: [{
    label: 'Files Modified',
    data: groupedByPath.value.map(([, count]) => count),
    backgroundColor: groupedByPath.value.map((_, i) => PALETTE[i % PALETTE.length]),
    borderWidth: 1
  }]
}))

const chartOptions = computed(() => ({
  indexAxis: 'y' as const,
  plugins: {
    legend: { display: false },
    title: {
      display: true,
      text: `Files modified by ${userName.value || '…'} — grouped at depth ${depth.value} (last ${lastNDays.value} days)`
    },
    tooltip: {
      callbacks: {
        label: (ctx: any) => ` ${ctx.parsed.x} file${ctx.parsed.x !== 1 ? 's' : ''}`
      }
    }
  },
  scales: { x: { beginAtZero: true, ticks: { precision: 0 } } }
}))

/** Files grouped for the detail table: prefix → sorted file list */
const groupedFiles = computed(() => {
  const map = new Map<string, TopFilesResponse[]>()
  for (const file of topFilesData.value) {
    const prefix = getPathPrefix(file.file_path, depth.value)
    if (!map.has(prefix)) map.set(prefix, [])
    map.get(prefix)!.push(file)
  }
  // Sort groups by count descending (same order as chart)
  return Array.from(map.entries()).sort((a, b) => b[1].length - a[1].length)
})

async function fetchTopFiles() {
  if (!userName.value) { error.value = 'Please provide a username'; return }
  loading.value = true
  error.value = null
  try {
    topFilesData.value = await getTopFiles({
      repository: repository.value,
      name: userName.value,
      top_n: topN.value,
      last_n_days: lastNDays.value || undefined
    })
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch top files'
    topFilesData.value = []
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="page-wrapper">
    <h1 class="mb-4">Modified Files by Path — Per User</h1>

    <b-card class="mb-4">
      <div class="row g-3">
        <div class="col-md-3">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" type="text" placeholder="owner/repo" />
        </div>
        <div class="col-md-3">
          <label class="form-label">Username</label>
          <b-form-input v-model="userName" type="text" placeholder="GitHub username" @keyup.enter="fetchTopFiles" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Fetch top N files</label>
          <b-form-input v-model.number="topN" type="number" min="1" max="500" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Last N Days</label>
          <b-form-input v-model.number="lastNDays" type="number" min="1" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Group depth</label>
          <b-form-select v-model.number="depth" :options="depthOptions" />
        </div>
      </div>
      <div class="mt-3">
        <b-button variant="primary" @click="fetchTopFiles" :disabled="loading">
          <b-spinner v-if="loading" small class="me-1" />
          {{ loading ? 'Loading…' : 'Fetch Data' }}
        </b-button>
        <span v-if="topFilesData.length" class="ms-3 text-muted small">
          {{ topFilesData.length }} files → {{ groupedByPath.length }} path groups
        </span>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">{{ error }}</b-alert>

    <template v-if="topFilesData.length > 0">
      <b-card class="mb-4">
        <bar-chart-component :data="chartData" :options="chartOptions" :height="Math.max(300, groupedByPath.length * 28)" />
      </b-card>

      <b-card>
        <h6 class="mb-3">Breakdown by path</h6>
        <div class="path-groups">
          <div v-for="([prefix, files], idx) in groupedFiles" :key="prefix" class="path-group">
            <div class="path-group-header" :style="{ borderLeftColor: PALETTE[idx % PALETTE.length] }">
              <code>{{ prefix }}/</code>
              <b-badge variant="primary" class="ms-2">{{ files.length }} file{{ files.length !== 1 ? 's' : '' }}</b-badge>
            </div>
            <ul class="file-list">
              <li v-for="file in files" :key="file.file_path">
                {{ file.file_path.slice(prefix.length + 1) || file.file_path }}
                <span class="text-muted small ms-2">PR #{{ file.pr_id }}</span>
              </li>
            </ul>
          </div>
        </div>
      </b-card>
    </template>

    <div v-else-if="!loading" class="text-muted text-center py-5">
      Enter a username and click "Fetch Data" to see files grouped by path
    </div>
  </div>
</template>

<style scoped>
.page-wrapper { padding: 2rem; max-width: 1400px; margin: 0 auto; }
.path-groups { display: flex; flex-direction: column; gap: 1rem; }
.path-group-header {
  padding: 0.4rem 0.75rem;
  background: #f8f9fa;
  border-left: 4px solid #dee2e6;
  border-radius: 0 4px 4px 0;
  display: flex;
  align-items: center;
}
.file-list {
  list-style: none;
  padding: 0.25rem 0 0 1.25rem;
  margin: 0;
  font-size: 0.85rem;
  color: #495057;
}
.file-list li { padding: 0.2rem 0; border-bottom: 1px solid #f1f3f5; }
.file-list li:last-child { border-bottom: none; }
.form-label { font-weight: 500; margin-bottom: 0.5rem; }
</style>
