<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert } from 'bootstrap-vue-next'
import BarChartComponent from '@/components/Charts/BarChartComponent.vue'
import { getTopFiles, TopFilesResponse } from '@/services/prApi'

const route = useRoute()
const router = useRouter()

const repository = ref<string>('rust-lang/rust')
const userName = ref<string>('')
const topN = ref<number>(10)
const duration = ref<number>(30)

const topFilesData = ref<TopFilesResponse[]>([])
const loading = ref(false)
const error = ref<string | null>(null)

// Load from URL on mount
onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.user) userName.value = String(route.query.user)
  if (route.query.top_n) topN.value = Number(route.query.top_n)
  if (route.query.duration) duration.value = Number(route.query.duration)

  if (userName.value) fetchTopFiles()
})

// Sync filters to URL
watch([repository, userName, topN, duration], () => {
  router.replace({
    query: {
      ...(repository.value ? { repository: repository.value } : {}),
      ...(userName.value ? { user: userName.value } : {}),
      top_n: topN.value,
      duration: duration.value
    }
  })
}, { flush: 'post' })

const chartData = computed(() => {
  const labels = topFilesData.value.map(f => f.file_path.split('/').pop() || f.file_path)
  const data = topFilesData.value.map((_, idx) => topFilesData.value.length - idx)
  return {
    labels,
    datasets: [{
      label: 'PR References',
      data,
      backgroundColor: [
        'rgba(54, 162, 235, 0.8)', 'rgba(255, 99, 132, 0.8)', 'rgba(255, 206, 86, 0.8)',
        'rgba(75, 192, 192, 0.8)', 'rgba(153, 102, 255, 0.8)', 'rgba(255, 159, 64, 0.8)',
        'rgba(201, 203, 207, 0.8)', 'rgba(83, 102, 255, 0.8)', 'rgba(255, 99, 255, 0.8)',
        'rgba(99, 255, 132, 0.8)'
      ],
      borderWidth: 1
    }]
  }
})

const chartOptions = computed(() => ({
  plugins: {
    legend: { display: false },
    title: { display: true, text: `Top ${topN.value} Modified Files (Last ${duration.value} days)` },
    tooltip: {
      callbacks: {
        title: (context: any) => {
          const item = topFilesData.value[context[0].dataIndex]
          return item ? `${item.file_path} (PR #${item.pr_id})` : ''
        }
      }
    }
  },
  scales: { y: { beginAtZero: true, ticks: { precision: 0 } } }
}))

async function fetchTopFiles() {
  if (!userName.value) { error.value = 'Please provide a username'; return }
  loading.value = true
  error.value = null
  try {
    topFilesData.value = await getTopFiles({
      repository: repository.value,
      name: userName.value,
      top_n: topN.value,
      duration: duration.value || undefined
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
    <h1 class="mb-4">Top Modified Files by User</h1>

    <b-card class="mb-4">
      <div class="row g-3">
        <div class="col-md-4">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" type="text" placeholder="owner/repo" />
        </div>
        <div class="col-md-4">
          <label class="form-label">Username</label>
          <b-form-input v-model="userName" type="text" placeholder="GitHub username" @keyup.enter="fetchTopFiles" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Top N</label>
          <b-form-input v-model.number="topN" type="number" min="1" max="50" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Duration (days)</label>
          <b-form-input v-model.number="duration" type="number" min="1" />
        </div>
      </div>
      <div class="mt-3">
        <b-button variant="primary" @click="fetchTopFiles" :disabled="loading">
          <b-spinner v-if="loading" small class="me-1" />
          {{ loading ? 'Loading...' : 'Fetch Data' }}
        </b-button>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">{{ error }}</b-alert>

    <b-card v-if="topFilesData.length > 0">
      <bar-chart-component :data="chartData" :options="chartOptions" :height="400" :horizontal="true" />
      <div class="mt-3">
        <h6>File Details:</h6>
        <ul class="file-list">
          <li v-for="(file, idx) in topFilesData" :key="idx">
            <strong>{{ file.file_path }}</strong> — PR #{{ file.pr_id }} by {{ file.pr_creator.github_name }}
          </li>
        </ul>
      </div>
    </b-card>
    <div v-else-if="!loading" class="text-muted text-center py-5">
      Enter a username and click "Fetch Data" to see top modified files
    </div>
  </div>
</template>

<style scoped>
.page-wrapper { padding: 2rem; max-width: 1400px; margin: 0 auto; }
.file-list { list-style: none; padding: 0; max-height: 300px; overflow-y: auto; }
.file-list li { padding: 0.5rem; border-bottom: 1px solid #e9ecef; }
.file-list li:last-child { border-bottom: none; }
.form-label { font-weight: 500; margin-bottom: 0.5rem; }
</style>
