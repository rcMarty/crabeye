<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert, BBadge } from 'bootstrap-vue-next'
import BarChartComponent from '@/components/Charts/BarChartComponent.vue'
import { getIssueEvents, getStatusType, getStatusTime, type PullRequestStatus, type PullRequestStatusType } from '@/services/prApi'

const route = useRoute()
const router = useRouter()

// Filters
const repository = ref<string>('rust-lang/rust')
const issueNumber = ref<string>('')
const timestamp = ref<string>('')

// Data
const events = ref<PullRequestStatus[]>([])
const loading = ref(false)
const error = ref<string | null>(null)
const fetched = ref(false)

// Sync URL → inputs on mount
onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.issue) issueNumber.value = String(route.query.issue)
  if (route.query.timestamp) timestamp.value = String(route.query.timestamp)
  else timestamp.value = new Date().toISOString().split('T')[0]

  if (issueNumber.value && timestamp.value) {
    fetchData()
  }
})

// Sync inputs → URL
watch([repository, issueNumber, timestamp], () => {
  router.replace({
    query: {
      ...(repository.value ? { repository: repository.value } : {}),
      ...(issueNumber.value ? { issue: issueNumber.value } : {}),
      ...(timestamp.value ? { timestamp: timestamp.value } : {})
    }
  })
}, { flush: 'post' })

// State metadata
const STATE_META: Record<PullRequestStatusType, { label: string; color: string; bg: string; icon: string }> = {
  Open:              { label: 'Open',               color: '#3b82f6', bg: 'primary',   icon: '🟢' },
  WaitingForReview:  { label: 'Waiting for Review', color: '#f59e0b', bg: 'warning',   icon: '👀' },
  WaitingForBors:    { label: 'Waiting for Bors',   color: '#8b5cf6', bg: 'info',      icon: '🤖' },
  WaitingForAuthor:  { label: 'Waiting for Author', color: '#06b6d4', bg: 'secondary', icon: '✏️' },
  Merged:            { label: 'Merged',              color: '#10b981', bg: 'success',   icon: '✅' },
  Closed:            { label: 'Closed',              color: '#ef4444', bg: 'danger',    icon: '❌' }
}

// Sorted timeline
const sortedEvents = computed(() =>
  [...events.value].sort((a, b) => {
    const ta = new Date(getStatusTime(a)).getTime()
    const tb = new Date(getStatusTime(b)).getTime()
    return ta - tb
  })
)

// State counts for bar chart
const stateCounts = computed(() => {
  const counts: Record<string, number> = {}
  for (const ev of events.value) {
    const t = getStatusType(ev)
    counts[t] = (counts[t] ?? 0) + 1
  }
  return counts
})

const barChartData = computed(() => {
  const types = Object.keys(stateCounts.value) as PullRequestStatusType[]
  return {
    labels: types.map(t => STATE_META[t]?.label ?? t),
    datasets: [{
      label: 'State occurrences',
      data: types.map(t => stateCounts.value[t]),
      backgroundColor: types.map(t => STATE_META[t]?.color ?? '#6c757d'),
      borderColor: types.map(t => STATE_META[t]?.color ?? '#6c757d'),
      borderWidth: 1,
      borderRadius: 6
    }]
  }
})

const barChartOptions = computed(() => ({
  responsive: true,
  plugins: {
    legend: { display: false },
    title: {
      display: true,
      text: `State Distribution — Issue #${issueNumber.value}`,
      font: { size: 15 }
    }
  },
  scales: {
    y: {
      beginAtZero: true,
      ticks: { stepSize: 1, precision: 0 }
    }
  }
}))

// Duration between consecutive events in sortedEvents
const stateSegments = computed(() => {
  const evs = sortedEvents.value
  return evs.map((ev, i) => {
    const type = getStatusType(ev)
    const start = new Date(getStatusTime(ev))
    const end = i + 1 < evs.length ? new Date(getStatusTime(evs[i + 1])) : null
    const durationMs = end ? end.getTime() - start.getTime() : null
    const durationHuman = durationMs != null ? formatDuration(durationMs) : 'current'
    return { type, start, end, durationMs, durationHuman, meta: STATE_META[type] }
  })
})

function formatDuration(ms: number): string {
  const s = Math.floor(ms / 1000)
  const m = Math.floor(s / 60)
  const h = Math.floor(m / 60)
  const d = Math.floor(h / 24)
  if (d > 0) return `${d}d ${h % 24}h`
  if (h > 0) return `${h}h ${m % 60}m`
  if (m > 0) return `${m}m`
  return `${s}s`
}

function formatTs(d: Date): string {
  return d.toLocaleString('cs-CZ', { dateStyle: 'short', timeStyle: 'short' })
}

async function fetchData() {
  if (!issueNumber.value || !timestamp.value) {
    error.value = 'Issue number and timestamp are required.'
    return
  }
  const num = parseInt(issueNumber.value, 10)
  if (isNaN(num) || num <= 0) {
    error.value = 'Issue number must be a positive integer.'
    return
  }

  loading.value = true
  error.value = null
  fetched.value = false
  try {
    events.value = await getIssueEvents({
      repository: repository.value,
      issue: num,
      timestamp: timestamp.value
    })
    fetched.value = true
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch issue events'
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="page-wrapper">
    <h1 class="page-title mb-1">
      <i class="pe-7s-ticket me-2"></i>Issue Events
    </h1>
    <p class="text-muted mb-4">View the state history of an issue/PR at a specific point in time.</p>

    <!-- Filters -->
    <b-card class="filter-card mb-4">
      <div class="row g-3 align-items-end">
        <div class="col-md-4">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" placeholder="owner/repo" />
        </div>
        <div class="col-md-3">
          <label class="form-label">Issue / PR Number <span class="text-danger">*</span></label>
          <b-form-input v-model="issueNumber" type="number" min="1" placeholder="e.g. 12345" />
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
    <div v-if="fetched && events.length === 0 && !loading" class="text-center py-5 text-muted">
      <div class="display-6 mb-2">📭</div>
      <p>No events found for issue <strong>#{{ issueNumber }}</strong> up to <strong>{{ timestamp }}</strong>.</p>
    </div>

    <!-- Results -->
    <div v-if="fetched && events.length > 0">
      <!-- Summary cards -->
      <div class="summary-grid mb-4">
        <div class="summary-card">
          <div class="summary-value">{{ events.length }}</div>
          <div class="summary-label">Total Events</div>
        </div>
        <div
          v-for="(count, state) in stateCounts"
          :key="state"
          class="summary-card"
          :style="{ borderTopColor: STATE_META[state as PullRequestStatusType]?.color }"
        >
          <div class="summary-value" :style="{ color: STATE_META[state as PullRequestStatusType]?.color }">
            {{ count }}
          </div>
          <div class="summary-label">{{ STATE_META[state as PullRequestStatusType]?.label ?? state }}</div>
        </div>
      </div>

      <div class="row g-4">
        <!-- Bar chart -->
        <div class="col-lg-6">
          <b-card class="h-100">
            <bar-chart-component :data="barChartData" :options="barChartOptions" :height="280" />
          </b-card>
        </div>

        <!-- Timeline -->
        <div class="col-lg-6">
          <b-card class="h-100">
            <h6 class="fw-semibold mb-3">State Timeline</h6>
            <div class="timeline-list">
              <div
                v-for="(seg, i) in stateSegments"
                :key="i"
                class="timeline-item"
              >
                <div class="timeline-dot" :style="{ background: seg.meta?.color }"></div>
                <div class="timeline-content">
                  <div class="d-flex align-items-center gap-2 mb-1">
                    <span class="state-icon">{{ seg.meta?.icon }}</span>
                    <!-- eslint-disable-next-line @typescript-eslint/no-explicit-any -->
                    <b-badge :variant="(seg.meta?.bg ?? 'secondary') as any" class="state-badge">
                      {{ seg.meta?.label ?? seg.type }}
                    </b-badge>
                    <span class="timeline-duration text-muted small">{{ seg.durationHuman }}</span>
                  </div>
                  <div class="timeline-time text-muted small">
                    {{ formatTs(seg.start) }}
                    <span v-if="seg.end"> → {{ formatTs(seg.end) }}</span>
                  </div>
                </div>
              </div>
            </div>
          </b-card>
        </div>
      </div>
    </div>

    <!-- Placeholder before fetch -->
    <div v-if="!fetched && !loading && !error" class="text-center py-5 text-muted">
      <div class="display-6 mb-2">🔍</div>
      <p>Enter an issue number and date, then click <strong>Fetch</strong> to load state history.</p>
    </div>
  </div>
</template>

<style scoped>
.page-wrapper {
  padding: 2rem;
  max-width: 1400px;
  margin: 0 auto;
}

.page-title {
  font-size: 1.6rem;
  font-weight: 700;
}

.filter-card {
  background: #fff;
  border: 1px solid #e9ecef;
}

/* Summary grid */
.summary-grid {
  display: flex;
  flex-wrap: wrap;
  gap: 1rem;
}

.summary-card {
  flex: 1 1 130px;
  background: #fff;
  border: 1px solid #e9ecef;
  border-top: 3px solid #69aa8a;
  border-radius: 0.5rem;
  padding: 1rem 1.25rem;
  text-align: center;
  box-shadow: 0 1px 4px rgba(0,0,0,.04);
}

.summary-value {
  font-size: 2rem;
  font-weight: 700;
  color: #343a40;
  line-height: 1.1;
}

.summary-label {
  font-size: 0.72rem;
  color: #6c757d;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-top: 0.25rem;
}

/* Timeline */
.timeline-list {
  position: relative;
  padding-left: 1.5rem;
}

.timeline-list::before {
  content: '';
  position: absolute;
  left: 8px;
  top: 4px;
  bottom: 4px;
  width: 2px;
  background: #e9ecef;
  border-radius: 2px;
}

.timeline-item {
  position: relative;
  padding-bottom: 1.25rem;
  padding-left: 1.25rem;
}

.timeline-item:last-child {
  padding-bottom: 0;
}

.timeline-dot {
  position: absolute;
  left: -1.5rem;
  top: 4px;
  width: 12px;
  height: 12px;
  border-radius: 50%;
  border: 2px solid #fff;
  box-shadow: 0 0 0 2px #dee2e6;
}

.timeline-content {
  background: #f8f9fa;
  border-radius: 0.4rem;
  padding: 0.6rem 0.85rem;
}

.state-icon {
  font-size: 1rem;
}

.state-badge {
  font-size: 0.72rem;
  font-weight: 600;
}

.timeline-duration {
  margin-left: auto;
  font-style: italic;
}
</style>
