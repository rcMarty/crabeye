<script lang="ts" setup>
import { ref, computed, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { BCard, BButton, BFormInput, BSpinner, BAlert, BFormSelect, BFormCheckbox } from 'bootstrap-vue-next'
import BarChartComponent from '@/components/Charts/BarChartComponent.vue'
import TreemapChartComponent from '@/components/Charts/TreemapChartComponent.vue'
import { JsonViewer } from 'vue3-json-viewer'
import 'vue3-json-viewer/dist/vue3-json-viewer.css'
import {
  getFilesModifiedByTeam,
  getTeams,
  GroupingLevel,
  FilesModifiedResponse,
  FileNode,
  DEFAULT_REPOSITORY
} from '@/services/prApi'

const route = useRoute()
const router = useRouter()

const repository = ref<string>(DEFAULT_REPOSITORY)
const teamName = ref<string>('')
const teamAnchorDate = ref<string>('')
const teamLastNDays = ref<number>(30)
const teamGroupLevel = ref<GroupingLevel>(null)
const teamFilesData = ref<FilesModifiedResponse | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const showJsonView = ref(false)
const jsonExpandDepth = ref<number>(3)
const jsonViewerKey = ref<number>(0)
const teamOptions = ref<string[]>([])

const groupLevelOptions = [
  { value: null, text: 'Auto (null)' },
  { value: 'none', text: 'No grouping (flat list)' },
  { value: 1, text: 'Level 1 (top level folders)' },
  { value: 2, text: 'Level 2 (subfolders)' },
  { value: 3, text: 'Level 3' },
  { value: 4, text: 'Level 4' },
  { value: 5, text: 'Level 5' },
  { value: 6, text: 'Level 6' },
  { value: 7, text: 'Level 7' },
  { value: 8, text: 'Level 8' },
  { value: 'all', text: 'All levels (full tree)' }
]

onMounted(async () => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.team) teamName.value = String(route.query.team)
  if (route.query.from) teamAnchorDate.value = String(route.query.from)
  if (route.query.days) teamLastNDays.value = Number(route.query.days)
  if (route.query.group_level !== undefined) {
    const gl = route.query.group_level
    if (gl === 'none' || gl === 'all') teamGroupLevel.value = gl as GroupingLevel
    else if (gl === 'null' || gl === '') teamGroupLevel.value = null
    else teamGroupLevel.value = Number(gl) as GroupingLevel
  }

  // Load teams list for the selector
  try { teamOptions.value = await getTeams() } catch { /* ignore */ }

  if (teamName.value) fetchTeamFiles()
})

watch([repository, teamName, teamAnchorDate, teamLastNDays, teamGroupLevel], () => {
  router.replace({
    query: {
      ...(repository.value ? { repository: repository.value } : {}),
      ...(teamName.value ? { team: teamName.value } : {}),
      ...(teamAnchorDate.value ? { from: teamAnchorDate.value } : {}),
      days: teamLastNDays.value,
      group_level: teamGroupLevel.value === null ? 'null' : String(teamGroupLevel.value)
    }
  })
}, { flush: 'post' })

const isTeamFilesFlat = computed(() => {
  if (!teamFilesData.value) return false
  return teamFilesData.value.type === 'list'
})

const teamFilesEntries = computed(() => {
  if (!teamFilesData.value || teamFilesData.value.type !== 'list') return []
  return Object.entries(teamFilesData.value.data)
})

// Flat list search and pagination
const flatSearchTerm = ref('')
const flatPage = ref(1)
const flatPerPage = ref(25)

const filteredFlatEntries = computed(() => {
  const q = flatSearchTerm.value.trim().toLowerCase()
  if (!q) return teamFilesEntries.value
  return teamFilesEntries.value.filter(([file]) => file.toLowerCase().includes(q))
})

const flatTotalPages = computed(() => Math.max(1, Math.ceil(filteredFlatEntries.value.length / flatPerPage.value)))

const paginatedFlatEntries = computed(() => {
  const start = (flatPage.value - 1) * flatPerPage.value
  return filteredFlatEntries.value.slice(start, start + flatPerPage.value)
})

const flatMaxCount = computed(() => {
  if (teamFilesEntries.value.length === 0) return 1
  return teamFilesEntries.value[0]?.[1] || 1
})

// Reset page when search changes
watch(flatSearchTerm, () => { flatPage.value = 1 })

const teamFilesChartData = computed(() => {
  if (!isTeamFilesFlat.value) return null
  const entries = teamFilesEntries.value
  return {
    labels: entries.map(([file]) => file.split('/').pop() || file),
    datasets: [{
      label: 'Modifications',
      data: entries.map(([, count]) => count),
      backgroundColor: 'rgba(54, 162, 235, 0.8)',
      borderColor: 'rgba(54, 162, 235, 1)',
      borderWidth: 1
    }]
  }
})

const chartOptions = computed(() => ({
  plugins: {
    legend: { display: false },
    title: {
      display: true,
      text: `Files Modified by Team: ${teamName.value}` + (teamLastNDays.value ? ` (Last ${teamLastNDays.value} days)` : '')
    }
  },
  scales: { y: { beginAtZero: true, ticks: { precision: 0 } } }
}))

const teamFilesTreeData = computed(() => {
  if (!teamFilesData.value || teamFilesData.value.type !== 'tree') return null
  return teamFilesData.value.data
})

const getJsonFieldTypeRank = (value: unknown): number => {
  if (Array.isArray(value)) return 1
  if (value !== null && typeof value === 'object') return 2
  return 0
}

const orderJsonByFieldType = (value: unknown): unknown => {
  if (Array.isArray(value)) return value.map(item => orderJsonByFieldType(item))
  if (value !== null && typeof value === 'object') {
    const entries = Object.entries(value as Record<string, unknown>)
    const sortedEntries = entries.sort(([keyA, valueA], [keyB, valueB]) => {
      const rankDiff = getJsonFieldTypeRank(valueA) - getJsonFieldTypeRank(valueB)
      return rankDiff !== 0 ? rankDiff : keyA.localeCompare(keyB)
    })
    return Object.fromEntries(sortedEntries.map(([key, v]) => [key, orderJsonByFieldType(v)]))
  }
  return value
}

const orderedTeamFilesTreeData = computed(() =>
  teamFilesTreeData.value ? orderJsonByFieldType(teamFilesTreeData.value) : null
)

function handleJsonKeyClick(keyName: string) {
  if (keyName === 'children') {
    jsonExpandDepth.value = 100
    jsonViewerKey.value++
  }
}

async function fetchTeamFiles() {
  if (!teamName.value) { error.value = 'Please provide team name'; return }

  loading.value = true
  error.value = null
  jsonExpandDepth.value = 3
  jsonViewerKey.value++
  flatPage.value = 1
  flatSearchTerm.value = ''
  try {
    teamFilesData.value = await getFilesModifiedByTeam({
      repository: repository.value,
      team_name: teamName.value,
      anchor_date: teamAnchorDate.value || undefined,
      last_n_days: teamLastNDays.value || undefined,
      group_level: teamGroupLevel.value
    })
  } catch (err) {
    error.value = err instanceof Error ? err.message : 'Failed to fetch team files'
    teamFilesData.value = null
  } finally {
    loading.value = false
  }
}
</script>

<template>
  <div class="page-wrapper">
    <h1 class="mb-4">Files Modified by Team</h1>

    <b-card class="mb-4">
      <div class="row g-3">
        <div class="col-md-3">
          <label class="form-label">Repository</label>
          <b-form-input v-model="repository" type="text" placeholder="owner/repo" />
        </div>
        <div class="col-md-3">
          <label class="form-label">Team Name</label>
          <b-form-select v-if="teamOptions.length > 0" v-model="teamName">
            <option value="">Select team...</option>
            <option v-for="t in teamOptions" :key="t" :value="t">{{ t }}</option>
          </b-form-select>
          <b-form-input v-else v-model="teamName" type="text" placeholder="Enter team name" @keyup.enter="fetchTeamFiles" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Anchor Date (optional)</label>
          <b-form-input v-model="teamAnchorDate" type="date" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Last N Days (optional)</label>
          <b-form-input v-model.number="teamLastNDays" type="number" min="1" />
        </div>
        <div class="col-md-2">
          <label class="form-label">Grouping Depth</label>
          <b-form-select v-model="teamGroupLevel" :options="groupLevelOptions" />
        </div>
      </div>
      <div class="mt-3">
        <b-button variant="primary" @click="fetchTeamFiles" :disabled="loading">
          <b-spinner v-if="loading" small class="me-1" />
          {{ loading ? 'Loading...' : 'Fetch Data' }}
        </b-button>
      </div>
    </b-card>

    <b-alert v-if="error" variant="danger" show dismissible @dismissed="error = null">{{ error }}</b-alert>

    <b-card v-if="teamFilesData">
      <!-- Flat list (bar chart + table) -->
      <template v-if="isTeamFilesFlat">
        <bar-chart-component
          :data="teamFilesChartData || { labels: [], datasets: [] }"
          :options="chartOptions"
          :height="Math.max(300, Math.min(teamFilesEntries.length * 22, 600))"
          :horizontal="true"
        />

        <div class="mt-4">
          <div class="d-flex justify-content-between align-items-center mb-3">
            <h6 class="mb-0">
              File Details
              <span class="text-muted fw-normal ms-2 small">({{ filteredFlatEntries.length }} of {{ teamFilesEntries.length }} files)</span>
            </h6>
            <div class="d-flex gap-2 align-items-center">
              <input
                v-model="flatSearchTerm"
                type="text"
                class="form-control form-control-sm"
                placeholder="Filter files…"
                style="width: 220px;"
              />
              <select v-model.number="flatPerPage" class="form-select form-select-sm" style="width: 80px;">
                <option :value="10">10</option>
                <option :value="25">25</option>
                <option :value="50">50</option>
                <option :value="100">100</option>
              </select>
            </div>
          </div>

          <div class="table-responsive">
            <table class="table table-sm table-hover file-table mb-0">
              <thead>
                <tr>
                  <th style="width: 60px;">#</th>
                  <th>File Path</th>
                  <th style="width: 150px;">Modifications</th>
                  <th style="width: 200px;"></th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="([file, count], idx) in paginatedFlatEntries" :key="file">
                  <td class="text-muted">{{ (flatPage - 1) * flatPerPage + idx + 1 }}</td>
                  <td>
                    <code class="file-path">{{ file }}</code>
                  </td>
                  <td>
                    <strong>{{ count }}</strong>
                  </td>
                  <td>
                    <div class="progress" style="height: 8px;">
                      <div
                        class="progress-bar"
                        role="progressbar"
                        :style="{ width: (count / flatMaxCount * 100) + '%' }"
                        :aria-valuenow="count"
                        :aria-valuemax="flatMaxCount"
                      ></div>
                    </div>
                  </td>
                </tr>
              </tbody>
            </table>
          </div>

          <div v-if="flatTotalPages > 1" class="d-flex justify-content-between align-items-center mt-3">
            <small class="text-muted">
              Showing {{ (flatPage - 1) * flatPerPage + 1 }}–{{ Math.min(flatPage * flatPerPage, filteredFlatEntries.length) }}
              of {{ filteredFlatEntries.length }}
            </small>
            <div class="d-flex gap-1 align-items-center">
              <button class="btn btn-sm btn-outline-secondary" :disabled="flatPage <= 1" @click="flatPage--">
                ‹ Prev
              </button>
              <template v-for="p in flatTotalPages" :key="p">
                <button
                  v-if="p === 1 || p === flatTotalPages || (p >= flatPage - 2 && p <= flatPage + 2)"
                  class="btn btn-sm"
                  :class="p === flatPage ? 'btn-primary' : 'btn-outline-secondary'"
                  @click="flatPage = p"
                >{{ p }}</button>
                <span v-else-if="p === flatPage - 3 || p === flatPage + 3" class="px-1 text-muted">…</span>
              </template>
              <button class="btn btn-sm btn-outline-secondary" :disabled="flatPage >= flatTotalPages" @click="flatPage++">
                Next ›
              </button>
            </div>
          </div>
        </div>
      </template>

      <!-- Hierarchical tree -->
      <template v-else>
        <div class="d-flex justify-content-between align-items-center mb-3">
          <h6 class="mb-0">Visualization</h6>
          <b-form-checkbox v-model="showJsonView" switch>Show JSON</b-form-checkbox>
        </div>

        <div v-if="showJsonView" class="json-view">
          <JsonViewer
            :key="jsonViewerKey"
            :value="orderedTeamFilesTreeData"
            :expand-depth="jsonExpandDepth"
            copyable
            boxed
            @onKeyClick="handleJsonKeyClick"
          />
        </div>
        <div v-else>
          <treemap-chart-component :data="(teamFilesTreeData || {})" :height="900" />
          <div v-if="teamFilesTreeData" class="mt-3 tree-summary">
            <p><strong>Root:</strong> {{ teamFilesTreeData.name }}</p>
            <p><strong>Total Modifications:</strong> {{ teamFilesTreeData.modifications }}</p>
            <p class="mb-0"><strong>Direct Children:</strong> {{ teamFilesTreeData.children?.length || 0 }}</p>
          </div>
        </div>
      </template>
    </b-card>
    <div v-else-if="!loading" class="text-muted text-center py-5">
      Enter team name, select grouping level, and click "Fetch Data"
    </div>
  </div>
</template>

<style scoped>
.page-wrapper { padding: 2rem; max-width: 1400px; margin: 0 auto; }
.form-label { font-weight: 500; margin-bottom: 0.5rem; }
.file-table code.file-path {
  font-size: 0.82rem;
  color: #495057;
  background: #f1f3f5;
  padding: 0.15rem 0.4rem;
  border-radius: 3px;
  word-break: break-all;
}
.file-table td { vertical-align: middle; }
.file-table .progress { background: #e9ecef; border-radius: 4px; }
.file-table .progress-bar { background: rgba(54, 162, 235, 0.8); border-radius: 4px; transition: width 0.3s; }
.tree-summary { padding: 1rem; background: #f8f9fa; border-radius: 0.5rem; margin-top: 1rem; }
.json-view { max-height: 600px; overflow: auto; border-radius: 0.5rem; }
.json-view :deep(.jv-container) { font-size: 0.875rem; }
.json-view :deep(.jv-code) { padding: 1rem; }
</style>
