<script lang="ts" setup>
import { ref, computed, onMounted } from 'vue'
import { BCard, BButton, BFormInput, BSpinner, BAlert } from 'bootstrap-vue-next'
import BarChartComponent from '@/components/Charts/BarChartComponent.vue'
import PieChartComponent from '@/components/Charts/PieChartComponent.vue'
import DoughnutChartComponent from '@/components/Charts/DoughnutChartComponent.vue'
import {
  getTopFiles,
  getPrsInState,
  getWaitingForReview,
  TopFilesResponse,
  PullRequestStatusType,
  PrEvent,
  getStatusType
} from '@/services/prApi'

// Repository configuration
const repository = ref<string>('rust-lang/rust')

// Top Files Chart State
const userName = ref<string>('')
const topN = ref<number>(10)
const duration = ref<number>(30)
const topFilesData = ref<TopFilesResponse[]>([])
const loadingTopFiles = ref(false)
const topFilesError = ref<string | null>(null)

// PRs in State Chart State
const selectedDate = ref<string>('')
const prsStateData = ref<Record<PullRequestStatusType, number>>({
  Open: 0,
  Closed: 0,
  Merged: 0,
  WaitingForBors: 0,
  WaitingForReview: 0,
  WaitingForAuthor: 0
})
const loadingPrsState = ref(false)
const prsStateError = ref<string | null>(null)

// Waiting for Review State
const waitingPrs = ref<PrEvent[]>([])
const loadingWaiting = ref(false)
const waitingError = ref<string | null>(null)

// Top Files Chart Data
const topFilesChartData = computed(() => {
  const labels = topFilesData.value.map(f => f.file_path.split('/').pop() || f.file_path)
  // Since API returns list of PRs touching files, count is based on number of items
  const data = topFilesData.value.map((_, idx) => topFilesData.value.length - idx)

  return {
    labels,
    datasets: [{
      label: 'PR References',
      data,
      backgroundColor: [
        'rgba(54, 162, 235, 0.8)',
        'rgba(255, 99, 132, 0.8)',
        'rgba(255, 206, 86, 0.8)',
        'rgba(75, 192, 192, 0.8)',
        'rgba(153, 102, 255, 0.8)',
        'rgba(255, 159, 64, 0.8)',
        'rgba(201, 203, 207, 0.8)',
        'rgba(83, 102, 255, 0.8)',
        'rgba(255, 99, 255, 0.8)',
        'rgba(99, 255, 132, 0.8)'
      ],
      borderColor: [
        'rgba(54, 162, 235, 1)',
        'rgba(255, 99, 132, 1)',
        'rgba(255, 206, 86, 1)',
        'rgba(75, 192, 192, 1)',
        'rgba(153, 102, 255, 1)',
        'rgba(255, 159, 64, 1)',
        'rgba(201, 203, 207, 1)',
        'rgba(83, 102, 255, 1)',
        'rgba(255, 99, 255, 1)',
        'rgba(99, 255, 132, 1)'
      ],
      borderWidth: 1
    }]
  }
})

const topFilesChartOptions = {
  plugins: {
    legend: {
      display: false
    },
    title: {
      display: true,
      text: `Top ${topN.value} Modified Files (Last ${duration.value} days)`
    },
    tooltip: {
      callbacks: {
        title: function(context: any) {
          const index = context[0].dataIndex
          const item = topFilesData.value[index]
          return item ? `${item.file_path} (PR #${item.pr_id})` : ''
        }
      }
    }
  },
  scales: {
    y: {
      beginAtZero: true,
      ticks: {
        precision: 0
      }
    }
  }
}

// PRs State Chart Data
const prsStateChartData = computed(() => {
  return {
    labels: ['Open', 'Closed', 'Merged'],
    datasets: [{
      label: 'Pull Requests',
      data: [
        prsStateData.value.Open,
        prsStateData.value.Closed,
        prsStateData.value.Merged
      ],
      backgroundColor: [
        'rgba(54, 162, 235, 0.8)',
        'rgba(255, 99, 132, 0.8)',
        'rgba(75, 192, 192, 0.8)'
      ],
      borderColor: [
        'rgba(54, 162, 235, 1)',
        'rgba(255, 99, 132, 1)',
        'rgba(75, 192, 192, 1)'
      ],
      borderWidth: 1
    }]
  }
})

const prsStateChartOptions = {
  plugins: {
    legend: {
      display: true,
      position: 'bottom'
    },
    title: {
      display: true,
      text: selectedDate.value ? `PR Status on ${selectedDate.value}` : 'PR Status (Current)'
    }
  }
}

// Waiting for Review Chart Data
const waitingChartData = computed(() => {
  return {
    labels: ['Waiting for Review', 'Others'],
    datasets: [{
      label: 'Pull Requests',
      data: [waitingPrs.value.length, 0], // We only have waiting count
      backgroundColor: [
        'rgba(255, 206, 86, 0.8)',
        'rgba(201, 203, 207, 0.8)'
      ],
      borderColor: [
        'rgba(255, 206, 86, 1)',
        'rgba(201, 203, 207, 1)'
      ],
      borderWidth: 1
    }]
  }
})

const waitingChartOptions = {
  plugins: {
    legend: {
      display: false
    },
    title: {
      display: true,
      text: 'PRs Waiting for Review'
    }
  }
}

// Fetch Functions
async function fetchTopFiles() {
  if (!userName.value || !topN.value) {
    topFilesError.value = 'Please provide User Name and Top N value'
    return
  }

  loadingTopFiles.value = true
  topFilesError.value = null

  try {
    topFilesData.value = await getTopFiles({
      repository: repository.value,
      name: userName.value,
      top_n: topN.value,
      duration: duration.value || undefined
    })
  } catch (err) {
    topFilesError.value = err instanceof Error ? err.message : 'Failed to fetch top files'
    topFilesData.value = []
  } finally {
    loadingTopFiles.value = false
  }
}

async function fetchPrsInState() {
  loadingPrsState.value = true
  prsStateError.value = null

  try {
    const states: PullRequestStatusType[] = ['Open', 'Closed', 'Merged', 'WaitingForAuthor', 'WaitingForBors', 'WaitingForReview']
    const results = await Promise.all(
      states.map(state =>
        getPrsInState({
          repository: repository.value,
          state,
          timestamp: selectedDate.value || null
        })
      )
    )

    prsStateData.value = {
      Open: results[0],
      Closed: results[1],
      Merged: results[2],
      WaitingForAuthor: results[3],
      WaitingForBors: results[4],
      WaitingForReview: results[5]
    }
  } catch (err) {
    prsStateError.value = err instanceof Error ? err.message : 'Failed to fetch PR states'
  } finally {
    loadingPrsState.value = false
  }
}

async function fetchWaitingForReview() {
  loadingWaiting.value = true
  waitingError.value = null

  try {
    const response = await getWaitingForReview({
      repository: repository.value
    })
    waitingPrs.value = response.items
  } catch (err) {
    waitingError.value = err instanceof Error ? err.message : 'Failed to fetch waiting PRs'
    waitingPrs.value = []
  } finally {
    loadingWaiting.value = false
  }
}

// Initialize with current date
onMounted(() => {
  const today = new Date().toISOString().split('T')[0]
  selectedDate.value = today
})
</script>

<template>
  <div class="pr-analytics-page">
    <h1 class="mb-4">Pull Request Analytics</h1>

    <!-- Repository Configuration -->
    <b-card class="mb-4">
      <div class="row">
        <div class="col-md-12">
          <label class="form-label">Repository</label>
          <b-form-input
            v-model="repository"
            type="text"
            placeholder="owner/repo (e.g., rust-lang/rust)"
          />
        </div>
      </div>
    </b-card>

    <!-- Top Files Chart -->
    <b-card class="mb-4">
      <template #header>
        <div class="d-flex justify-content-between align-items-center">
          <h5 class="mb-0">Top Modified Files by User</h5>
          <b-button
            size="sm"
            variant="primary"
            @click="fetchTopFiles"
            :disabled="loadingTopFiles"
          >
            <b-spinner v-if="loadingTopFiles" small class="me-1" />
            {{ loadingTopFiles ? 'Loading...' : 'Fetch Data' }}
          </b-button>
        </div>
      </template>

      <div class="row mb-3">
        <div class="col-md-4">
          <label class="form-label">User Name</label>
          <b-form-input
            v-model="userName"
            type="text"
            placeholder="Enter GitHub username"
          />
        </div>
        <div class="col-md-4">
          <label class="form-label">Top N Files</label>
          <b-form-input
            v-model.number="topN"
            type="number"
            placeholder="Number of top files"
            min="1"
            max="50"
          />
        </div>
        <div class="col-md-4">
          <label class="form-label">Duration (days)</label>
          <b-form-input
            v-model.number="duration"
            type="number"
            placeholder="Days to look back"
            min="1"
          />
        </div>
      </div>

      <b-alert v-if="topFilesError" variant="danger" show dismissible @dismissed="topFilesError = null">
        {{ topFilesError }}
      </b-alert>

      <div v-if="topFilesData.length > 0" class="chart-wrapper">
        <bar-chart-component
          :data="topFilesChartData"
          :options="topFilesChartOptions"
          :height="400"
          :horizontal="true"
        />

        <div class="mt-3">
          <h6>File Details:</h6>
          <ul class="file-list">
            <li v-for="(file, idx) in topFilesData" :key="idx">
              <strong>{{ file.file_path }}</strong> - PR #{{ file.pr_id }} by {{ file.pr_creator.github_name }}
            </li>
          </ul>
        </div>
      </div>
      <div v-else-if="!loadingTopFiles" class="text-muted text-center py-4">
        Enter user name and click "Fetch Data" to see top modified files
      </div>
    </b-card>

    <!-- PRs in State Chart -->
    <b-card class="mb-4">
      <template #header>
        <div class="d-flex justify-content-between align-items-center">
          <h5 class="mb-0">Pull Request Status Distribution</h5>
          <b-button
            size="sm"
            variant="primary"
            @click="fetchPrsInState"
            :disabled="loadingPrsState"
          >
            <b-spinner v-if="loadingPrsState" small class="me-1" />
            {{ loadingPrsState ? 'Loading...' : 'Fetch Data' }}
          </b-button>
        </div>
      </template>

      <div class="row mb-3">
        <div class="col-md-6">
          <label class="form-label">Date (optional - defaults to today)</label>
          <b-form-input
            v-model="selectedDate"
            type="date"
            placeholder="YYYY-MM-DD"
          />
        </div>
      </div>

      <b-alert v-if="prsStateError" variant="danger" show dismissible @dismissed="prsStateError = null">
        {{ prsStateError }}
      </b-alert>

      <div v-if="prsStateData.Open > 0 || prsStateData.Closed > 0 || prsStateData.Merged > 0" class="row">
        <div class="col-md-6">
          <pie-chart-component
            :data="prsStateChartData"
            :options="prsStateChartOptions"
            :height="300"
          />
        </div>
        <div class="col-md-6">
          <div class="stats-grid mt-4">
            <div class="stat-card">
              <div class="stat-label">Open</div>
              <div class="stat-value text-primary">{{ prsStateData.Open }}</div>
            </div>
            <div class="stat-card">
              <div class="stat-label">Closed</div>
              <div class="stat-value text-danger">{{ prsStateData.Closed }}</div>
            </div>
            <div class="stat-card">
              <div class="stat-label">Merged</div>
              <div class="stat-value text-success">{{ prsStateData.Merged }}</div>
            </div>
            <div class="stat-card">
              <div class="stat-label">Waiting (Review)</div>
              <div class="stat-value text-warning">{{ prsStateData.WaitingForReview }}</div>
            </div>
            <div class="stat-card">
              <div class="stat-label">Waiting (Author)</div>
              <div class="stat-value text-info">{{ prsStateData.WaitingForAuthor }}</div>
            </div>
            <div class="stat-card">
              <div class="stat-label">Waiting (Bors)</div>
              <div class="stat-value text-secondary">{{ prsStateData.WaitingForBors }}</div>
            </div>
          </div>
        </div>
      </div>
      <div v-else-if="!loadingPrsState" class="text-muted text-center py-4">
        Click "Fetch Data" to see PR status distribution
      </div>
    </b-card>

    <!-- Waiting for Review -->
    <b-card class="mb-4">
      <template #header>
        <div class="d-flex justify-content-between align-items-center">
          <h5 class="mb-0">PRs Waiting for Review</h5>
          <b-button
            size="sm"
            variant="primary"
            @click="fetchWaitingForReview"
            :disabled="loadingWaiting"
          >
            <b-spinner v-if="loadingWaiting" small class="me-1" />
            {{ loadingWaiting ? 'Loading...' : 'Fetch Data' }}
          </b-button>
        </div>
      </template>

      <b-alert v-if="waitingError" variant="danger" show dismissible @dismissed="waitingError = null">
        {{ waitingError }}
      </b-alert>

      <div v-if="waitingPrs.length > 0" class="row">
        <div class="col-md-6">
          <doughnut-chart-component
            :data="waitingChartData"
            :options="waitingChartOptions"
            :height="300"
          />
        </div>
        <div class="col-md-6">
          <div class="waiting-count">
            <div class="display-4">{{ waitingPrs.length }}</div>
            <div class="text-muted">PRs waiting for review</div>
          </div>

          <div class="mt-3">
            <h6>PR Numbers:</h6>
            <div class="pr-list">
              <span v-for="(pr, idx) in waitingPrs" :key="idx" class="badge bg-warning text-dark me-1 mb-1">
                #{{ pr.pr_number }} ({{ getStatusType(pr.state) }})
              </span>
            </div>
          </div>
        </div>
      </div>
      <div v-else-if="!loadingWaiting" class="text-muted text-center py-4">
        Click "Fetch Data" to see PRs waiting for review
      </div>
    </b-card>
  </div>
</template>

<style scoped>
.pr-analytics-page {
  padding: 2rem;
  max-width: 1400px;
  margin: 0 auto;
}

.chart-wrapper {
  margin-top: 1rem;
}

.file-list {
  list-style: none;
  padding: 0;
  max-height: 300px;
  overflow-y: auto;
}

.file-list li {
  padding: 0.5rem;
  border-bottom: 1px solid #e9ecef;
}

.file-list li:last-child {
  border-bottom: none;
}

.stats-grid {
  display: grid;
  grid-template-columns: repeat(3, 1fr);
  gap: 1rem;
}

@media (max-width: 768px) {
  .stats-grid {
    grid-template-columns: repeat(2, 1fr);
  }
}

.stat-card {
  padding: 1rem;
  background: #f8f9fa;
  border-radius: 0.5rem;
  text-align: center;
}

.stat-label {
  font-size: 0.75rem;
  color: #6c757d;
  margin-bottom: 0.5rem;
}

.stat-value {
  font-size: 1.5rem;
  font-weight: bold;
}

.waiting-count {
  text-align: center;
  padding: 2rem;
  background: #f8f9fa;
  border-radius: 0.5rem;
}

.pr-list {
  max-height: 200px;
  overflow-y: auto;
  padding: 0.5rem;
  background: #f8f9fa;
  border-radius: 0.25rem;
}

.form-label {
  font-weight: 500;
  margin-bottom: 0.5rem;
}
</style>
