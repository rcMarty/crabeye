<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert, BBadge } from 'bootstrap-vue-next'
import BarChartComponent from '@/components/Charts/BarChartComponent.vue'
import {
  getPrHistory,
  getStatusType,
  getStatusTime,
  type PrEvent,
  type PullRequestStatusType,
  type IssueEvent,
  type IssueLabel
} from '@/services/prApi'

const route = useRoute()
const router = useRouter()

// --- Filter state ---
const repository = ref<string>('rust-lang/rust')
const issueNumber = ref<string>('')
const timestamp = ref<string>('')

// --- Data ---
const prEvent = ref<PrEvent | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const fetched = ref(false)

// --- URL sync: read on mount ---
onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.issue)      issueNumber.value = String(route.query.issue)
  if (route.query.timestamp)  timestamp.value  = String(route.query.timestamp)
  else                        timestamp.value  = new Date().toISOString().split('T')[0]

  if (issueNumber.value && timestamp.value) fetchData()
})

// --- URL sync: write on change ---
watch([repository, issueNumber, timestamp], () => {
  router.replace({
    query: {
      ...(repository.value  ? { repository: repository.value }     : {}),
      ...(issueNumber.value ? { issue:      issueNumber.value }     : {}),
      ...(timestamp.value   ? { timestamp:  timestamp.value }       : {})
    }
  })
}, { flush: 'post' })

// --- Status metadata ---
const STATE_META: Record<PullRequestStatusType, { label: string; variant: string; color: string }> = {
  Open:             { label: 'Open',               variant: 'primary',   color: '#3b82f6' },
  WaitingForReview: { label: 'Waiting for Review', variant: 'warning',   color: '#f59e0b' },
  WaitingForBors:   { label: 'Waiting for Bors',   variant: 'info',      color: '#8b5cf6' },
  WaitingForAuthor: { label: 'Waiting for Author', variant: 'secondary', color: '#06b6d4' },
  Merged:           { label: 'Merged',              variant: 'success',   color: '#10b981' },
  Closed:           { label: 'Closed',              variant: 'danger',    color: '#ef4444' }
}

// --- Current state derived values ---
const currentStateType = computed<PullRequestStatusType | null>(() =>
  prEvent.value ? getStatusType(prEvent.value.state) : null
)
const currentStateMeta = computed(() =>
  currentStateType.value ? STATE_META[currentStateType.value] : null
)
const currentStateTime = computed(() =>
  prEvent.value ? getStatusTime(prEvent.value.state) : null
)

// --- Events history ---
const sortedEvents = computed<IssueEvent[]>(() =>
  [...(prEvent.value?.events_history ?? [])].sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
  )
)

const eventCounts = computed(() => {
  const counts: Record<string, number> = {}
  for (const ev of sortedEvents.value) {
    counts[ev.event] = (counts[ev.event] ?? 0) + 1
  }
  return counts
})

const eventsBarChartData = computed(() => {
  const keys = Object.keys(eventCounts.value)
  return {
    labels: keys,
    datasets: [{
      label: 'Occurrences',
      data: keys.map(k => eventCounts.value[k]),
      backgroundColor: '#3b82f6',
      borderColor: '#1d4ed8',
      borderWidth: 1,
      borderRadius: 5
    }]
  }
})

const eventsBarChartOptions = computed(() => ({
  responsive: true,
  plugins: {
    legend: { display: false },
    title: {
      display: true,
      text: `Event Type Distribution — PR #${issueNumber.value}`,
      font: { size: 14 }
    }
  },
  scales: {
    y: { beginAtZero: true, ticks: { stepSize: 1, precision: 0 } }
  }
}))

// --- Labels history ---
const sortedLabels = computed<IssueLabel[]>(() =>
  [...(prEvent.value?.labels_history ?? [])].sort(
    (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
  )
)

const labelStats = computed(() => {
  const map: Record<string, { added: number; removed: number }> = {}
  for (const l of sortedLabels.value) {
    if (!map[l.label]) map[l.label] = { added: 0, removed: 0 }
    if (l.action === 'ADDED')   map[l.label].added++
    else                        map[l.label].removed++
  }
  return map
})

const labelsBarChartData = computed(() => {
  const labels = Object.keys(labelStats.value)
  return {
    labels,
    datasets: [
      {
        label: 'Added',
        data: labels.map(l => labelStats.value[l].added),
        backgroundColor: '#10b981',
        borderColor: '#059669',
        borderWidth: 1,
        borderRadius: 4
      },
      {
        label: 'Removed',
        data: labels.map(l => labelStats.value[l].removed),
        backgroundColor: '#ef4444',
        borderColor: '#dc2626',
        borderWidth: 1,
        borderRadius: 4
      }
    ]
  }
})

const labelsBarChartOptions = computed(() => ({
  responsive: true,
  plugins: {
    legend: { display: true, position: 'bottom' as const },
    title: {
      display: true,
      text: `Label Activity — PR #${issueNumber.value}`,
      font: { size: 14 }
    }
  },
  scales: {
    y: { beginAtZero: true, ticks: { stepSize: 1, precision: 0 } }
  }
}))

// --- Helpers ---
function formatTs(iso: string): string {
  return new Date(iso).toLocaleString('cs-CZ', { dateStyle: 'short', timeStyle: 'short' })
}

// Typed variant accessor (avoids template TS error with union type)
const currentStateVariant = computed((): any => currentStateMeta.value?.variant ?? 'secondary')

// --- Fetch ---
async function fetchData() {
  const num = parseInt(issueNumber.value, 10)
  if (!issueNumber.value || isNaN(num) || num <= 0) {
    error.value = 'Please provide a valid positive PR number.'
    return
  }
  if (!timestamp.value) {
    error.value = 'Please provide a date.'
    return
  }

  loading.value = true
  error.value = null
  fetched.value = false
  prEvent.value = null

  try {
    prEvent.value = await getPrHistory({
      repository: repository.value,
      issue: num,
      timestamp: timestamp.value
    })
    fetched.value = true
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch PR history'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="page-wrapper">
    <!-- Header -->
    <h1 class="page-title mb-1">
      <i class="pe-7s-way me-2"></i>PR History
    </h1>
    <p class="text-muted mb-4">
      View the state, events, and label history of a Pull Request at a specific point in time.
    </p>

    <!-- Filter card -->
    <b-card class="filter-card mb-4">
      <div class="row g-3 align-items-end">
        <div class="col-md-4">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" placeholder="owner/repo" />
        </div>
        <div class="col-md-3">
          <label class="form-label">PR Number <span class="text-danger">*</span></label>
          <b-form-input
            v-model="issueNumber"
            type="number"
            min="1"
            placeholder="e.g. 12345"
            @keyup.enter="fetchData"
          />
        </div>
        <div class="col-md-3">
          <label class="form-label">Date <span class="text-danger">*</span></label>
          <b-form-input v-model="timestamp" type="date" />
        </div>
        <div class="col-md-2">
          <b-button variant="primary" class="w-100" @click="fetchData" :disabled="loading">
            <b-spinner v-if="loading" small class="me-1" />
            {{ loading ? 'Loading…' : 'Fetch' }}
          </b-button>
        </div>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">
      {{ error }}
    </b-alert>

    <!-- Empty state -->
    <div v-if="!fetched && !loading" class="text-center py-5 text-muted">
      <i class="pe-7s-way" style="font-size: 3rem; opacity: 0.3;"></i>
      <p class="mt-2">Enter a PR number and date, then click "Fetch"</p>
    </div>

    <!-- Results -->
    <template v-if="prEvent && fetched">

      <!-- Summary row -->
      <div class="row g-3 mb-4">
        <!-- State card -->
        <div class="col-md-4">
          <b-card class="stat-card h-100">
            <div class="stat-card-label">Current State</div>
            <div class="stat-card-value mt-2">
              <b-badge
                v-if="currentStateMeta"
                :variant="currentStateVariant"
                class="fs-6 px-3 py-2"
              >
                {{ currentStateMeta.label }}
              </b-badge>
            </div>
            <div v-if="currentStateTime" class="text-muted small mt-2">
              since {{ formatTs(currentStateTime) }}
            </div>
          </b-card>
        </div>

        <!-- PR meta card -->
        <div class="col-md-4">
          <b-card class="stat-card h-100">
            <div class="stat-card-label">PR Details</div>
            <div class="stat-card-value mt-2">
              <span class="fs-5 fw-bold">#{{ prEvent.pr_number }}</span>
            </div>
            <div class="text-muted small mt-1">Author ID: {{ prEvent.author_id }}</div>
            <div class="text-muted small">Repository: {{ prEvent.repository }}</div>
          </b-card>
        </div>

        <!-- Activity summary -->
        <div class="col-md-4">
          <b-card class="stat-card h-100">
            <div class="stat-card-label">Activity Summary</div>
            <div class="mt-2 d-flex gap-3 flex-wrap">
              <div class="activity-stat">
                <div class="activity-stat-value text-primary">{{ sortedEvents.length }}</div>
                <div class="activity-stat-label">Events</div>
              </div>
              <div class="activity-stat">
                <div class="activity-stat-value text-success">{{ sortedLabels.length }}</div>
                <div class="activity-stat-label">Label changes</div>
              </div>
              <div class="activity-stat">
                <div class="activity-stat-value text-info">{{ Object.keys(labelStats).length }}</div>
                <div class="activity-stat-label">Unique labels</div>
              </div>
            </div>
          </b-card>
        </div>
      </div>

      <!-- Events section -->
      <div v-if="sortedEvents.length > 0" class="row g-3 mb-4">
        <!-- Events chart -->
        <div class="col-md-6">
          <b-card class="h-100">
            <bar-chart-component
              :data="eventsBarChartData"
              :options="eventsBarChartOptions"
              :height="280"
            />
          </b-card>
        </div>

        <!-- Events timeline -->
        <div class="col-md-6">
          <b-card class="h-100">
            <h6 class="mb-3">Events Timeline</h6>
            <div class="timeline-scroll">
              <div
                v-for="(ev, idx) in sortedEvents"
                :key="idx"
                class="timeline-item"
              >
                <div class="timeline-dot"></div>
                <div class="timeline-content">
                  <span class="timeline-event-name">{{ ev.event }}</span>
                  <span class="timeline-ts text-muted ms-2 small">{{ formatTs(ev.timestamp) }}</span>
                </div>
              </div>
            </div>
          </b-card>
        </div>
      </div>
      <b-card v-else-if="fetched" class="mb-4 text-muted text-center py-3">
        No events history recorded for this PR.
      </b-card>

      <!-- Labels section -->
      <div v-if="sortedLabels.length > 0" class="row g-3 mb-4">
        <!-- Labels chart -->
        <div class="col-md-6">
          <b-card class="h-100">
            <bar-chart-component
              :data="labelsBarChartData"
              :options="labelsBarChartOptions"
              :height="280"
            />
          </b-card>
        </div>

        <!-- Labels table -->
        <div class="col-md-6">
          <b-card class="h-100">
            <h6 class="mb-3">Label Change Log</h6>
            <div class="labels-scroll">
              <table class="table table-sm table-hover mb-0">
                <thead>
                  <tr>
                    <th>Label</th>
                    <th>Action</th>
                    <th>Date</th>
                  </tr>
                </thead>
                <tbody>
                  <tr v-for="(lbl, idx) in sortedLabels" :key="idx">
                    <td>
                      <b-badge variant="secondary">{{ lbl.label }}</b-badge>
                    </td>
                    <td>
                      <b-badge :variant="lbl.action === 'ADDED' ? 'success' : 'danger'">
                        {{ lbl.action }}
                      </b-badge>
                    </td>
                    <td class="text-muted small">{{ formatTs(lbl.timestamp) }}</td>
                  </tr>
                </tbody>
              </table>
            </div>
          </b-card>
        </div>
      </div>
      <b-card v-else-if="fetched" class="mb-4 text-muted text-center py-3">
        No label history recorded for this PR.
      </b-card>

    </template>
  </div>
</template>

<style scoped>
.page-wrapper {
  padding: 2rem;
  max-width: 1400px;
  margin: 0 auto;
}

.form-label {
  font-weight: 500;
  margin-bottom: 0.4rem;
}

/* Stat cards */
.stat-card {
  background: #f8f9fa;
  border: 1px solid #e9ecef;
}

.stat-card-label {
  font-size: 0.8rem;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: #6c757d;
  font-weight: 600;
}

.stat-card-value {
  font-size: 1.5rem;
  font-weight: 700;
  line-height: 1.2;
}

/* Activity summary */
.activity-stat {
  text-align: center;
  min-width: 60px;
}

.activity-stat-value {
  font-size: 1.4rem;
  font-weight: 700;
  line-height: 1;
}

.activity-stat-label {
  font-size: 0.7rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: #6c757d;
  margin-top: 0.2rem;
}

/* Timeline */
.timeline-scroll {
  max-height: 320px;
  overflow-y: auto;
  padding-right: 0.25rem;
}

.timeline-item {
  display: flex;
  align-items: flex-start;
  gap: 0.75rem;
  padding: 0.5rem 0;
  border-bottom: 1px solid #f0f0f0;
}

.timeline-item:last-child {
  border-bottom: none;
}

.timeline-dot {
  width: 10px;
  height: 10px;
  min-width: 10px;
  border-radius: 50%;
  background: #3b82f6;
  margin-top: 0.35rem;
}

.timeline-content {
  flex: 1;
  font-size: 0.9rem;
}

.timeline-event-name {
  font-weight: 500;
}

/* Labels table */
.labels-scroll {
  max-height: 320px;
  overflow-y: auto;
}

.table th {
  font-size: 0.78rem;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: #6c757d;
  border-top: none;
}
</style>
