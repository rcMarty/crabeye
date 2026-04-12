<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert } from 'bootstrap-vue-next'
import PieChartComponent from '@/components/Charts/PieChartComponent.vue'
import { getPrsInState, PullRequestStatusType, DEFAULT_REPOSITORY } from '@/services/prApi'
import { formatDateEU, toIsoDate } from '@/utils/dateFormat'

const route = useRoute()
const router = useRouter()

const repository = ref<string>(DEFAULT_REPOSITORY)
const selectedDate = ref<string>('')

const prsStateData = ref<Record<PullRequestStatusType, number>>({
  Open: 0, Closed: 0, Merged: 0, WaitingForBors: 0, WaitingForReview: 0, WaitingForAuthor: 0
})
const loading = ref(false)
const error = ref<string | null>(null)
const hasData = computed(() =>
  prsStateData.value.Open > 0 || prsStateData.value.Closed > 0 || prsStateData.value.Merged > 0
)

onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.date) selectedDate.value = toIsoDate(String(route.query.date)) || ''
  else selectedDate.value = new Date().toISOString().split('T')[0]
})

watch([repository, selectedDate], () => {
  router.replace({
    query: {
      ...(repository.value ? { repository: repository.value } : {}),
      ...(selectedDate.value ? { date: selectedDate.value } : {})
    }
  })
}, { flush: 'post' })

const chartData = computed(() => ({
  labels: ['Open', 'Closed', 'Merged'],
  datasets: [{
    label: 'Pull Requests',
    data: [prsStateData.value.Open, prsStateData.value.Closed, prsStateData.value.Merged],
    backgroundColor: ['rgba(54,162,235,0.8)', 'rgba(255,99,132,0.8)', 'rgba(75,192,192,0.8)'],
    borderColor: ['rgba(54,162,235,1)', 'rgba(255,99,132,1)', 'rgba(75,192,192,1)'],
    borderWidth: 1
  }]
}))

const chartOptions = computed(() => ({
  plugins: {
    legend: { display: true, position: 'bottom' },
    title: {
      display: true,
      text: selectedDate.value ? `PR Status on ${formatDateEU(selectedDate.value)}` : 'PR Status (Current)'
    }
  }
}))

const stats = computed(() => [
  { label: 'Open', value: prsStateData.value.Open, cls: 'text-primary' },
  { label: 'Closed', value: prsStateData.value.Closed, cls: 'text-danger' },
  { label: 'Merged', value: prsStateData.value.Merged, cls: 'text-success' },
  { label: 'Waiting (Review)', value: prsStateData.value.WaitingForReview, cls: 'text-warning' },
  { label: 'Waiting (Author)', value: prsStateData.value.WaitingForAuthor, cls: 'text-info' },
  { label: 'Waiting (Bors)', value: prsStateData.value.WaitingForBors, cls: 'text-secondary' }
])

async function fetch() {
  const isoDate = toIsoDate(selectedDate.value)
  if (selectedDate.value && !isoDate) {
    error.value = 'Invalid date format. Use dd/mm/yyyy'
    return
  }

  loading.value = true
  error.value = null
  try {
    const states: PullRequestStatusType[] = ['Open', 'Closed', 'Merged', 'WaitingForAuthor', 'WaitingForBors', 'WaitingForReview']
    const results = await Promise.all(
      states.map(state => getPrsInState({ repository: repository.value, state, timestamp: isoDate || null }))
    )
    prsStateData.value = {
      Open: results[0], Closed: results[1], Merged: results[2],
      WaitingForAuthor: results[3], WaitingForBors: results[4], WaitingForReview: results[5]
    }
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch PR states'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="page-wrapper">
    <h1 class="mb-4">Pull Request Status Distribution</h1>

    <b-card class="mb-4">
      <div class="row g-3">
        <div class="col-md-6">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" type="text" placeholder="owner/repo" />
        </div>
        <div class="col-md-6">
          <label class="form-label">Date (optional — defaults to today)</label>
          <b-form-input v-model="selectedDate" type="date" />
        </div>
      </div>
      <div class="mt-3">
        <b-button variant="primary" @click="fetch" :disabled="loading">
          <b-spinner v-if="loading" small class="me-1" />
          {{ loading ? 'Loading...' : 'Fetch Data' }}
        </b-button>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">{{ error }}</b-alert>

    <b-card v-if="hasData">
      <div class="row">
        <div class="col-md-6">
          <pie-chart-component :data="chartData" :options="chartOptions" :height="300" />
        </div>
        <div class="col-md-6">
          <div class="stats-grid mt-4">
            <div v-for="s in stats" :key="s.label" class="stat-card">
              <div class="stat-label">{{ s.label }}</div>
              <div class="stat-value" :class="s.cls">{{ s.value }}</div>
            </div>
          </div>
        </div>
      </div>
    </b-card>
    <div v-else-if="!loading" class="text-muted text-center py-5">
      Click "Fetch Data" to see PR status distribution
    </div>
  </div>
</template>

<style scoped>
.page-wrapper { padding: 2rem; max-width: 1400px; margin: 0 auto; }
.form-label { font-weight: 500; margin-bottom: 0.5rem; }
.stats-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 1rem; }
@media (max-width: 768px) { .stats-grid { grid-template-columns: repeat(2, 1fr); } }
.stat-card { padding: 1rem; background: #f8f9fa; border-radius: 0.5rem; text-align: center; }
.stat-label { font-size: 0.75rem; color: #6c757d; margin-bottom: 0.5rem; }
.stat-value { font-size: 1.5rem; font-weight: bold; }
</style>
