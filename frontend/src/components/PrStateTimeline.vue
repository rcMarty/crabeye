<script lang="ts" setup>
import { ref, computed } from 'vue'
import { BCard, BButton, BFormInput, BSpinner, BAlert } from 'bootstrap-vue-next'
import LineChartComponent from '@/components/Charts/LineChartComponent.vue'
import { getPrHistory, PrEvent, getStatusType } from '@/services/prApi'
import { formatDateTimeEU, toIsoDate } from '@/utils/dateFormat'

const repository = ref<string>('rust-lang/rust')
const prNumber = ref<number | null>(null)
const timestamp = ref<string>('')
const prEventData = ref<PrEvent | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)

const chartData = computed(() => {
  if (!prEventData.value || !prEventData.value.events_history || prEventData.value.events_history.length === 0) {
    return {
      labels: [],
      datasets: []
    }
  }

  const labels = prEventData.value.events_history.map(entry => formatDateTimeEU(entry.timestamp))
  const stateMap: Record<string, number> = {
    'open': 0,
    'closed': 1,
    'merged': 2,
    'reopened': 0,
    'ready_for_review': 0
  }

  const data = prEventData.value.events_history.map(entry => {
    return stateMap[entry.event.toLowerCase()] ?? 0
  })

  return {
    labels,
    datasets: [{
      label: 'PR Events',
      data,
      borderColor: 'rgba(75, 192, 192, 1)',
      backgroundColor: 'rgba(75, 192, 192, 0.2)',
      tension: 0.1,
      fill: true
    }]
  }
})

const chartOptions = {
  plugins: {
    legend: {
      display: false
    },
    title: {
      display: true,
      text: `PR #${prNumber.value || ''} Event Timeline`
    }
  },
  scales: {
    y: {
      beginAtZero: true,
      ticks: {
        callback: function(value: number) {
          const states = ['Open', 'Closed', 'Merged']
          return states[value] || value
        },
        stepSize: 1
      }
    },
    x: {
      title: {
        display: true,
        text: 'Timestamp'
      }
    }
  }
}

async function fetchPrState() {
  if (!prNumber.value || !timestamp.value || !repository.value) {
    error.value = 'Please provide repository, PR number and timestamp'
    return
  }

  const timestampIsoDate = toIsoDate(timestamp.value)
  if (!timestampIsoDate) {
    error.value = 'Invalid timestamp format. Use dd/mm/yyyy'
    return
  }

  loading.value = true
  error.value = null

  try {
    prEventData.value = await getPrHistory({
      repository: repository.value,
      issue: prNumber.value,
      timestamp: timestampIsoDate
    })
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch PR history'
    prEventData.value = null
  } finally {
    loading.value = false
  }
}

// Initialize with current date
const today = new Date().toISOString().split('T')[0]
timestamp.value = today
</script>

<template>
  <div class="pr-state-timeline">
    <b-card>
      <template #header>
        <div class="d-flex justify-content-between align-items-center">
          <h5 class="mb-0">PR State Timeline</h5>
          <b-button
            size="sm"
            variant="primary"
            @click="fetchPrState"
            :disabled="loading"
          >
            <b-spinner v-if="loading" small class="me-1" />
            {{ loading ? 'Loading...' : 'Fetch State' }}
          </b-button>
        </div>
      </template>

      <div class="row mb-3">
        <div class="col-md-4">
          <label class="form-label">Repository</label>
          <b-form-input
            v-model="repository"
            type="text"
            placeholder="owner/repo (e.g., rust-lang/rust)"
          />
        </div>
        <div class="col-md-4">
          <label class="form-label">PR Number</label>
          <b-form-input
            v-model.number="prNumber"
            type="number"
            placeholder="Enter PR number (e.g., 123)"
            min="1"
          />
        </div>
        <div class="col-md-4">
          <label class="form-label">Timestamp</label>
          <b-form-input
            v-model="timestamp"
            type="date"
          />
        </div>
      </div>

      <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">
        {{ error }}
      </b-alert>

      <div v-if="prEventData && prEventData.events_history && prEventData.events_history.length > 0" class="chart-section">
        <line-chart-component
          :data="chartData"
          :options="chartOptions"
          :height="350"
        />

        <div class="mt-3">
          <h6>Current State: <span :class="`badge bg-${getStateBadgeColor(getStatusType(prEventData.state))}`">{{ getStatusType(prEventData.state) }}</span></h6>

          <h6 class="mt-3">Event History:</h6>
          <ul class="state-list">
            <li v-for="(entry, idx) in prEventData.events_history" :key="idx">
              <strong>{{ formatDateTimeEU(entry.timestamp) }}</strong>:
              <span :class="`badge bg-${getStateBadgeColor(entry.event)}`">
                {{ entry.event }}
              </span>
            </li>
          </ul>

          <div v-if="prEventData.labels_history && prEventData.labels_history.length > 0" class="mt-3">
            <h6>Label History:</h6>
            <ul class="state-list">
              <li v-for="(label, idx) in prEventData.labels_history" :key="idx">
                <strong>{{ formatDateTimeEU(label.timestamp) }}</strong>:
                <span :class="`badge bg-${label.action === 'ADDED' ? 'success' : 'danger'}`">
                  {{ label.action }} - {{ label.label }}
                </span>
              </li>
            </ul>
          </div>
        </div>
      </div>
      <div v-else-if="!loading" class="text-muted text-center py-4">
        Enter repository, PR number and timestamp, then click "Fetch State" to see the timeline
      </div>
    </b-card>
  </div>
</template>

<script lang="ts">
function getStateBadgeColor(state: string): string {
  const stateLower = state.toLowerCase()
  switch (stateLower) {
  case 'open':
    return 'primary'
  case 'closed':
    return 'danger'
  case 'merged':
    return 'success'
  case 'draft':
    return 'warning'
  default:
    return 'secondary'
  }
}
</script>

<style scoped>
.pr-state-timeline {
  padding: 1rem;
}

.chart-section {
  margin-top: 1rem;
}

.state-list {
  list-style: none;
  padding: 0;
  max-height: 250px;
  overflow-y: auto;
}

.state-list li {
  padding: 0.5rem;
  border-bottom: 1px solid #e9ecef;
}

.state-list li:last-child {
  border-bottom: none;
}

.state-list .badge {
  margin-left: 0.5rem;
  font-size: 0.875rem;
}

.form-label {
  font-weight: 500;
  margin-bottom: 0.5rem;
}
</style>
