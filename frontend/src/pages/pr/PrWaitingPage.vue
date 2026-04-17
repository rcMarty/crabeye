<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert } from 'bootstrap-vue-next'
import DoughnutChartComponent from '@/components/Charts/DoughnutChartComponent.vue'
import {
  getWaitingForReview,
  getPrsInState,
  type PrEvent,
  type PaginatedResponse,
  type PullRequestStatusType,
  getStatusType,
  getStatusTime,
  DEFAULT_REPOSITORY
} from '@/services/prApi'

const route = useRoute()
const router = useRouter()

const repository = ref<string>(DEFAULT_REPOSITORY)
const allPrs = ref<PrEvent[]>([])
const totalCount = ref(0)
const loading = ref(false)
const error = ref<string | null>(null)

// State counts for the doughnut chart
const stateCounts = ref<Record<string, number>>({})
const loadingStates = ref(false)

// Pagination for the PR list display
const listPage = ref(1)
const listPerPage = ref(25)

onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (repository.value) fetchAll()
})

watch([repository], () => {
  router.replace({
    query: { ...(repository.value ? { repository: repository.value } : {}) }
  })
}, { flush: 'post' })

const STATE_META: Record<string, { label: string; color: string; bg: string }> = {
  WaitingForReview: { label: 'Waiting for Review', color: '#f59e0b', bg: 'rgba(245,158,11,0.8)' },
  WaitingForAuthor: { label: 'Waiting for Author', color: '#06b6d4', bg: 'rgba(6,182,212,0.8)' },
  WaitingForBors:   { label: 'Waiting for Bors',   color: '#8b5cf6', bg: 'rgba(139,92,246,0.8)' },
  Open:             { label: 'Open',                color: '#3b82f6', bg: 'rgba(59,130,246,0.8)' },
  Closed:           { label: 'Closed',              color: '#ef4444', bg: 'rgba(239,68,68,0.8)' },
  Merged:           { label: 'Merged',              color: '#10b981', bg: 'rgba(16,185,129,0.8)' }
}

// Chart for all states
const chartData = computed(() => {
  const entries = Object.entries(stateCounts.value).filter(([, v]) => v > 0)
  return {
    labels: entries.map(([k]) => STATE_META[k]?.label ?? k),
    datasets: [{
      data: entries.map(([, v]) => v),
      backgroundColor: entries.map(([k]) => STATE_META[k]?.bg ?? '#999'),
      borderColor: entries.map(([k]) => STATE_META[k]?.color ?? '#666'),
      borderWidth: 2
    }]
  }
})

const chartOptions = {
  plugins: {
    legend: { display: true, position: 'bottom' as const },
    title: { display: true, text: 'PR State Distribution (Current)' }
  }
}

// Sort PRs by state time ascending (oldest first — longest waiting)
const sortedPrs = computed(() => {
  return [...allPrs.value].sort((a, b) => {
    const ta = new Date(getStatusTime(a.state)).getTime()
    const tb = new Date(getStatusTime(b.state)).getTime()
    return ta - tb
  })
})

// Paginated view
const paginatedPrs = computed(() => {
  const start = (listPage.value - 1) * listPerPage.value
  return sortedPrs.value.slice(start, start + listPerPage.value)
})

const totalPages = computed(() => Math.max(1, Math.ceil(allPrs.value.length / listPerPage.value)))

function waitingDuration(pr: PrEvent): string {
  const stateTime = getStatusTime(pr.state)
  if (!stateTime) return '—'
  const ms = Date.now() - new Date(stateTime).getTime()
  const days = Math.floor(ms / 86400000)
  const hours = Math.floor((ms % 86400000) / 3600000)
  if (days > 0) return `${days}d ${hours}h`
  return `${hours}h`
}

function formatDate(iso: string): string {
  if (!iso) return '—'
  return new Date(iso).toLocaleDateString('en-GB', { day: 'numeric', month: 'short', year: 'numeric' })
}

async function fetchAll() {
  loading.value = true
  error.value = null
  listPage.value = 1

  try {
    // Fetch ALL waiting PRs by paginating through
    const pageSize = 100
    let page = 1
    let collected: PrEvent[] = []
    let total = 0

    while (true) {
      const resp: PaginatedResponse<PrEvent> = await getWaitingForReview({
        repository: repository.value,
        pagination: { page, per_page: pageSize }
      })
      total = resp.total_count
      collected = collected.concat(resp.items)
      if (collected.length >= total || resp.items.length < pageSize) break
      page++
    }

    allPrs.value = collected
    totalCount.value = total

    // Fetch state counts for the doughnut chart
    loadingStates.value = true
    const states: PullRequestStatusType[] = ['Open', 'Closed', 'Merged', 'WaitingForReview', 'WaitingForAuthor', 'WaitingForBors']
    const results = await Promise.all(
      states.map(state => getPrsInState({ repository: repository.value, state }))
    )
    const counts: Record<string, number> = {}
    states.forEach((s, i) => { counts[s] = results[i].count })
    stateCounts.value = counts
    loadingStates.value = false
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch waiting PRs'
    allPrs.value = []
    totalCount.value = 0
  } finally {
    loading.value = false
    loadingStates.value = false
  }
}
</script>

<template>
  <div class="page-wrapper">
    <h1 class="page-title mb-1"><i class="pe-7s-clock me-2"></i>PRs Waiting for Review</h1>
    <p class="text-muted mb-4">All open pull requests currently in a waiting state, sorted by how long they've been waiting.</p>

    <b-card class="mb-4">
      <div class="row g-3 align-items-end">
        <div class="col-md-8">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" type="text" placeholder="owner/repo" />
        </div>
        <div class="col-md-4">
          <b-button variant="primary" class="w-100" @click="fetchAll" :disabled="loading">
            <b-spinner v-if="loading" small class="me-1" />
            {{ loading ? 'Loading...' : 'Fetch Data' }}
          </b-button>
        </div>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">{{ error }}</b-alert>

    <template v-if="allPrs.length > 0 || Object.keys(stateCounts).length > 0">
      <!-- Top row: chart + summary -->
      <div class="row g-4 mb-4">
        <div class="col-md-5">
          <b-card class="h-100">
            <doughnut-chart-component :data="chartData" :options="chartOptions" :height="320" />
          </b-card>
        </div>
        <div class="col-md-7">
          <b-card class="h-100">
            <h6 class="mb-3">State Overview</h6>
            <div class="state-grid">
              <div
                v-for="(meta, key) in STATE_META"
                :key="key"
                class="state-card"
                :style="{ borderLeftColor: meta.color }"
              >
                <div class="state-count" :style="{ color: meta.color }">
                  {{ stateCounts[key] ?? '—' }}
                </div>
                <div class="state-label">{{ meta.label }}</div>
              </div>
            </div>
            <div class="mt-3 p-2 bg-light rounded">
              <strong class="text-warning">{{ totalCount }}</strong>
              <span class="text-muted ms-1">PRs currently waiting for review</span>
            </div>
          </b-card>
        </div>
      </div>

      <!-- PR list -->
      <b-card>
        <div class="d-flex justify-content-between align-items-center mb-3">
          <h6 class="mb-0">
            Waiting PRs — sorted by longest waiting
            <span class="text-muted fw-normal ms-2 small">({{ allPrs.length }} total)</span>
          </h6>
          <select v-model.number="listPerPage" class="form-select form-select-sm" style="width: 80px;">
            <option :value="10">10</option>
            <option :value="25">25</option>
            <option :value="50">50</option>
            <option :value="100">100</option>
          </select>
        </div>

        <div class="table-responsive">
          <table class="table table-sm table-hover mb-0">
            <thead>
              <tr>
                <th style="width: 50px;">#</th>
                <th>PR</th>
                <th>State</th>
                <th>In State Since</th>
                <th>Waiting</th>
                <th>Created</th>
              </tr>
            </thead>
            <tbody>
              <tr v-for="(pr, idx) in paginatedPrs" :key="pr.pr_number">
                <td class="text-muted">{{ (listPage - 1) * listPerPage + idx + 1 }}</td>
                <td>
                  <a
                    :href="`https://github.com/${repository}/pull/${pr.pr_number}`"
                    target="_blank"
                    class="fw-bold text-decoration-none"
                  >
                    #{{ pr.pr_number }}
                  </a>
                </td>
                <td>
                  <span
                    class="badge"
                    :style="{ backgroundColor: STATE_META[getStatusType(pr.state)]?.bg ?? '#999' }"
                  >
                    {{ STATE_META[getStatusType(pr.state)]?.label ?? getStatusType(pr.state) }}
                  </span>
                </td>
                <td class="text-muted small">{{ formatDate(getStatusTime(pr.state)) }}</td>
                <td>
                  <span class="badge bg-dark">{{ waitingDuration(pr) }}</span>
                </td>
                <td class="text-muted small">{{ formatDate(pr.created_at) }}</td>
              </tr>
            </tbody>
          </table>
        </div>

        <div v-if="totalPages > 1" class="d-flex justify-content-between align-items-center mt-3">
          <small class="text-muted">
            Showing {{ (listPage - 1) * listPerPage + 1 }}–{{ Math.min(listPage * listPerPage, allPrs.length) }}
            of {{ allPrs.length }}
          </small>
          <div class="d-flex gap-1 align-items-center">
            <button class="btn btn-sm btn-outline-secondary" :disabled="listPage <= 1" @click="listPage--">‹ Prev</button>
            <template v-for="p in totalPages" :key="p">
              <button
                v-if="p === 1 || p === totalPages || (p >= listPage - 2 && p <= listPage + 2)"
                class="btn btn-sm"
                :class="p === listPage ? 'btn-primary' : 'btn-outline-secondary'"
                @click="listPage = p"
              >{{ p }}</button>
              <span v-else-if="p === listPage - 3 || p === listPage + 3" class="px-1 text-muted">…</span>
            </template>
            <button class="btn btn-sm btn-outline-secondary" :disabled="listPage >= totalPages" @click="listPage++">Next ›</button>
          </div>
        </div>
      </b-card>
    </template>

    <div v-else-if="!loading" class="text-muted text-center py-5">
      <i class="pe-7s-clock" style="font-size: 3rem; opacity: 0.3;"></i>
      <p class="mt-2">Click "Fetch Data" to see PRs waiting for review</p>
    </div>
  </div>
</template>

<style scoped>
.page-wrapper { padding: 2rem; max-width: 1400px; margin: 0 auto; }
.page-title { font-size: 1.6rem; font-weight: 700; }
.form-label { font-weight: 500; margin-bottom: 0.5rem; }
.state-grid { display: grid; grid-template-columns: repeat(3, 1fr); gap: 0.75rem; }
@media (max-width: 768px) { .state-grid { grid-template-columns: repeat(2, 1fr); } }
.state-card {
  padding: 0.75rem 1rem;
  background: #f8f9fa;
  border-left: 4px solid #dee2e6;
  border-radius: 0 0.375rem 0.375rem 0;
  text-align: center;
}
.state-count { font-size: 1.5rem; font-weight: 700; line-height: 1.1; }
.state-label { font-size: 0.7rem; color: #6c757d; text-transform: uppercase; letter-spacing: 0.04em; margin-top: 0.15rem; }
</style>
