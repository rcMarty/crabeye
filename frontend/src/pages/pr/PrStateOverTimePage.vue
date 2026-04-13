<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert, BFormSelect } from 'bootstrap-vue-next'
import LineChartComponent from '@/components/Charts/LineChartComponent.vue'
import {
  getPrsInStateOverTime,
  type DateCount,
  type PullRequestStatusType,
  type PrCountOverTimeResponse,
  DEFAULT_REPOSITORY
} from '@/services/prApi'
import { formatDateEU } from '@/utils/dateFormat'

const route = useRoute()
const router = useRouter()

const repository = ref<string>(DEFAULT_REPOSITORY)
const selectedState = ref<PullRequestStatusType>('WaitingForReview')
const anchorDate = ref<string>('')
const lastNDays = ref<number>(30)
const timeSeriesData = ref<DateCount[]>([])
const dataSince = ref<string | null>(null)
const dataTo = ref<string | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

// Combined chart state
const showCombined = ref(false)
const combinedData = ref<Record<PullRequestStatusType, DateCount[]>>({} as any)
const combinedSince = ref<string | null>(null)
const combinedTo = ref<string | null>(null)
const loadingCombined = ref(false)

const stateOptions = [
  { value: 'Open', text: 'Open' },
  { value: 'Closed', text: 'Closed' },
  { value: 'Merged', text: 'Merged' },
  { value: 'WaitingForReview', text: 'Waiting for Review' },
  { value: 'WaitingForAuthor', text: 'Waiting for Author' },
  { value: 'WaitingForBors', text: 'Waiting for Bors' }
]

const STATE_COLORS: Record<PullRequestStatusType, { line: string; fill: string }> = {
  Open:             { line: '#3b82f6', fill: 'rgba(59,130,246,0.15)' },
  Closed:           { line: '#ef4444', fill: 'rgba(239,68,68,0.15)' },
  Merged:           { line: '#10b981', fill: 'rgba(16,185,129,0.15)' },
  WaitingForReview: { line: '#f59e0b', fill: 'rgba(245,158,11,0.15)' },
  WaitingForAuthor: { line: '#06b6d4', fill: 'rgba(6,182,212,0.15)' },
  WaitingForBors:   { line: '#8b5cf6', fill: 'rgba(139,92,246,0.15)' }
}

onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.state) selectedState.value = String(route.query.state) as PullRequestStatusType
  if (route.query.anchor_date) anchorDate.value = String(route.query.anchor_date)
  if (route.query.last_n_days) lastNDays.value = Number(route.query.last_n_days)
})

watch([repository, selectedState, anchorDate, lastNDays], () => {
  router.replace({
    query: {
      ...(repository.value ? { repository: repository.value } : {}),
      state: selectedState.value,
      ...(anchorDate.value ? { anchor_date: anchorDate.value } : {}),
      last_n_days: lastNDays.value
    }
  })
}, { flush: 'post' })

const stateLabel = computed(() =>
  stateOptions.find(o => o.value === selectedState.value)?.text ?? selectedState.value
)

const chartData = computed(() => {
  const colors = STATE_COLORS[selectedState.value]
  return {
    labels: timeSeriesData.value.map(d => d.date),
    datasets: [{
      label: stateLabel.value,
      data: timeSeriesData.value.map(d => d.count),
      borderColor: colors.line,
      backgroundColor: colors.fill,
      fill: true,
      tension: 0.3,
      pointRadius: 3,
      pointHoverRadius: 6
    }]
  }
})

const chartOptions = computed(() => ({
  responsive: true,
  plugins: {
    legend: { display: true, position: 'bottom' as const },
    title: {
      display: true,
      text: `PRs in "${stateLabel.value}" state over ${lastNDays.value} days`,
      font: { size: 15 }
    },
    tooltip: {
      callbacks: {
        title: (ctx: any) => {
          const d = ctx[0]?.label
          return d ? new Date(d).toLocaleDateString('en-GB') : ''
        }
      }
    }
  },
  scales: {
    x: {
      type: 'category' as const,
      ticks: {
        maxTicksLimit: 15,
        callback: (_: unknown, index: number) => {
          const label = timeSeriesData.value[index]?.date
          return label ? new Date(label).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' }) : ''
        }
      }
    },
    y: { beginAtZero: true, ticks: { precision: 0 } }
  }
}))

// Summary stats
const summaryStats = computed(() => {
  if (timeSeriesData.value.length === 0) return null
  const counts = timeSeriesData.value.map(d => d.count)
  const min = Math.min(...counts)
  const max = Math.max(...counts)
  const avg = Math.round(counts.reduce((s, c) => s + c, 0) / counts.length)
  const latest = counts[counts.length - 1]
  const first = counts[0]
  const trend = latest - first
  return { min, max, avg, latest, trend }
})

async function fetchData() {
  loading.value = true
  error.value = null
  try {
    const response = await getPrsInStateOverTime({
      repository: repository.value,
      state: selectedState.value,
      anchor_date: anchorDate.value || undefined,
      last_n_days: lastNDays.value
    })
    timeSeriesData.value = response.data
    dataSince.value = response.since ?? null
    dataTo.value = response.to ?? null
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch time-series data'
    timeSeriesData.value = []
  } finally {
    loading.value = false
  }
}

const ALL_STATES: PullRequestStatusType[] = ['Open', 'Closed', 'Merged', 'WaitingForReview', 'WaitingForAuthor', 'WaitingForBors']

async function fetchCombinedData() {
  loadingCombined.value = true
  error.value = null
  try {
    const results = await Promise.all(
      ALL_STATES.map(state =>
        getPrsInStateOverTime({
          repository: repository.value,
          state,
          anchor_date: anchorDate.value || undefined,
          last_n_days: lastNDays.value
        })
      )
    )
    const data: Record<string, DateCount[]> = {}
    ALL_STATES.forEach((state, i) => { data[state] = results[i].data })
    combinedData.value = data as Record<PullRequestStatusType, DateCount[]>
    const firstWithSince = results.find(r => r.since)
    combinedSince.value = firstWithSince?.since ?? null
    combinedTo.value = results[0]?.to ?? null
    showCombined.value = true
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch combined time-series data'
  } finally {
    loadingCombined.value = false
  }
}

const combinedChartData = computed(() => {
  if (!showCombined.value || Object.keys(combinedData.value).length === 0) return null
  // Use labels from the first state that has data
  const firstData = Object.values(combinedData.value).find(d => d.length > 0)
  if (!firstData) return null

  return {
    labels: firstData.map(d => d.date),
    datasets: ALL_STATES.map(state => {
      const colors = STATE_COLORS[state]
      const stateData = combinedData.value[state] || []
      return {
        label: stateOptions.find(o => o.value === state)?.text ?? state,
        data: stateData.map(d => d.count),
        borderColor: colors.line,
        backgroundColor: colors.fill,
        fill: false,
        tension: 0.3,
        pointRadius: 2,
        pointHoverRadius: 5,
        borderWidth: 2
      }
    })
  }
})

const combinedChartOptions = computed(() => ({
  responsive: true,
  plugins: {
    legend: { display: true, position: 'bottom' as const },
    title: {
      display: true,
      text: `All PR States over ${lastNDays.value} days`,
      font: { size: 15 }
    },
    tooltip: {
      mode: 'index' as const,
      intersect: false,
      callbacks: {
        title: (ctx: any) => {
          const d = ctx[0]?.label
          return d ? new Date(d).toLocaleDateString('en-GB') : ''
        }
      }
    }
  },
  interaction: { mode: 'index' as const, intersect: false },
  scales: {
    x: {
      type: 'category' as const,
      ticks: {
        maxTicksLimit: 15,
        callback: (_: unknown, index: number) => {
          const firstData = Object.values(combinedData.value).find(d => d.length > 0)
          const label = firstData?.[index]?.date
          return label ? new Date(label).toLocaleDateString('en-GB', { day: 'numeric', month: 'short' }) : ''
        }
      }
    },
    y: { beginAtZero: true, ticks: { precision: 0 } }
  }
}))
</script>

<template>
  <div class="page-wrapper">
    <h1 class="page-title mb-1"><i class="pe-7s-graph3 me-2"></i>PR State Over Time</h1>
    <p class="text-muted mb-4">Daily count of pull requests in a specific state over a lookback window.</p>

    <b-card class="filter-card mb-4">
      <div class="row g-3 align-items-end">
        <div class="col-md-3">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" placeholder="owner/repo" />
        </div>
        <div class="col-md-3">
          <label class="form-label">PR State</label>
          <b-form-select v-model="selectedState" :options="stateOptions" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Anchor Date</label>
          <b-form-input v-model="anchorDate" type="date" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Last N Days (max 90)</label>
          <b-form-input v-model.number="lastNDays" type="number" min="1" max="90" />
        </div>
        <div class="col-md-2">
          <b-button variant="primary" class="w-100" @click="fetchData" :disabled="loading">
            <b-spinner v-if="loading" small class="me-1" />
            {{ loading ? 'Loading…' : 'Fetch' }}
          </b-button>
        </div>
      </div>
      <div class="mt-2">
        <b-button variant="outline-secondary" size="sm" @click="fetchCombinedData" :disabled="loadingCombined">
          <b-spinner v-if="loadingCombined" small class="me-1" />
          {{ loadingCombined ? 'Loading…' : 'Show All States Combined' }}
        </b-button>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">{{ error }}</b-alert>

    <template v-if="timeSeriesData.length > 0">
      <div v-if="dataSince" class="data-range-info mb-3">
        <small class="text-muted">
          <i class="pe-7s-info me-1"></i>
          Data available from <strong>{{ formatDateEU(dataSince) }}</strong>
          <template v-if="dataTo"> to <strong>{{ formatDateEU(dataTo) }}</strong></template>
        </small>
      </div>

      <!-- Summary cards -->
      <div v-if="summaryStats" class="summary-grid mb-4">
        <div class="summary-card">
          <div class="summary-value">{{ summaryStats.latest }}</div>
          <div class="summary-label">Current</div>
        </div>
        <div class="summary-card">
          <div class="summary-value">{{ summaryStats.avg }}</div>
          <div class="summary-label">Average</div>
        </div>
        <div class="summary-card">
          <div class="summary-value">{{ summaryStats.min }}</div>
          <div class="summary-label">Min</div>
        </div>
        <div class="summary-card">
          <div class="summary-value">{{ summaryStats.max }}</div>
          <div class="summary-label">Max</div>
        </div>
        <div class="summary-card" :class="summaryStats.trend > 0 ? 'trend-up' : summaryStats.trend < 0 ? 'trend-down' : ''">
          <div class="summary-value">
            {{ summaryStats.trend > 0 ? '+' : '' }}{{ summaryStats.trend }}
          </div>
          <div class="summary-label">Trend</div>
        </div>
      </div>

      <!-- Chart -->
      <b-card>
        <line-chart-component :data="chartData" :options="chartOptions" :height="400" />
      </b-card>
    </template>

    <div v-else-if="!loading" class="text-center py-5 text-muted">
      <i class="pe-7s-graph3" style="font-size: 3rem; opacity: 0.3;"></i>
      <p class="mt-2">Select a state and click "Fetch" to see the time-series.</p>
    </div>

    <!-- Combined all-states chart -->
    <template v-if="showCombined && combinedChartData">
      <hr class="my-4" />
      <h2 class="page-title mb-1"><i class="pe-7s-graph1 me-2"></i>All States Combined</h2>
      <p class="text-muted mb-3">Daily count of pull requests across all states in one view.</p>

      <div v-if="combinedSince" class="data-range-info mb-3">
        <small class="text-muted">
          <i class="pe-7s-info me-1"></i>
          Data available from <strong>{{ formatDateEU(combinedSince) }}</strong>
          <template v-if="combinedTo"> to <strong>{{ formatDateEU(combinedTo) }}</strong></template>
        </small>
      </div>

      <b-card>
        <line-chart-component :data="combinedChartData" :options="combinedChartOptions" :height="450" />
      </b-card>
    </template>
  </div>
</template>

<style scoped>
.page-wrapper { padding: 2rem; max-width: 1400px; margin: 0 auto; }
.page-title { font-size: 1.6rem; font-weight: 700; }
.filter-card { background: #fff; border: 1px solid #e9ecef; }
.form-label { font-weight: 500; margin-bottom: 0.4rem; }
.summary-grid { display: flex; flex-wrap: wrap; gap: 1rem; }
.summary-card {
  flex: 1 1 120px; background: #fff; border: 1px solid #e9ecef;
  border-top: 3px solid #3b82f6; border-radius: 0.5rem;
  padding: 1rem 1.25rem; text-align: center; box-shadow: 0 1px 4px rgba(0,0,0,.04);
}
.summary-value { font-size: 1.8rem; font-weight: 700; color: #343a40; line-height: 1.1; }
.summary-label { font-size: 0.72rem; color: #6c757d; text-transform: uppercase; letter-spacing: 0.05em; margin-top: 0.25rem; }
.trend-up { border-top-color: #ef4444; }
.trend-up .summary-value { color: #ef4444; }
.trend-down { border-top-color: #10b981; }
.trend-down .summary-value { color: #10b981; }
.data-range-info { padding: 0.5rem 0.75rem; background: #f0f7ff; border-left: 3px solid #3b82f6; border-radius: 0 0.25rem 0.25rem 0; }
</style>
