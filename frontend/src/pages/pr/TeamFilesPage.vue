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
  GroupingLevel,
  FilesModifiedResponse,
  FileNode
} from '@/services/prApi'
import { toIsoDate } from '@/utils/dateFormat'

const route = useRoute()
const router = useRouter()

const repository = ref<string>('rust-lang/rust')
const teamName = ref<string>('')
const teamFromTimestamp = ref<string>('')
const teamLastNDays = ref<number>(30)
const teamGroupLevel = ref<GroupingLevel>(null)
const teamFilesData = ref<FilesModifiedResponse | null>(null)
const loading = ref(false)
const error = ref<string | null>(null)
const showJsonView = ref(false)
const jsonExpandDepth = ref<number>(0)
const jsonViewerKey = ref<number>(0)

const groupLevelOptions = [
  { value: null, text: 'Auto (null)' },
  { value: 'none', text: 'No grouping (flat list)' },
  { value: 1, text: 'Level 1 (top level folders)' },
  { value: 2, text: 'Level 2 (subfolders)' },
  { value: 3, text: 'Level 3' },
  { value: 'all', text: 'All levels (full tree)' }
]

const isFileNode = (value: unknown): value is FileNode =>
  value !== null &&
  typeof value === 'object' &&
  'name' in value &&
  'modifications' in value &&
  'children' in value

onMounted(() => {
  if (route.query.repository) repository.value = String(route.query.repository)
  if (route.query.team) teamName.value = String(route.query.team)
  if (route.query.from) teamFromTimestamp.value = toIsoDate(String(route.query.from)) || ''
  if (route.query.days) teamLastNDays.value = Number(route.query.days)
  if (route.query.group_level !== undefined) {
    const gl = route.query.group_level
    if (gl === 'none' || gl === 'all') teamGroupLevel.value = gl as GroupingLevel
    else if (gl === 'null' || gl === '') teamGroupLevel.value = null
    else teamGroupLevel.value = Number(gl) as GroupingLevel
  }

  if (teamName.value) fetchTeamFiles()
})

watch([repository, teamName, teamFromTimestamp, teamLastNDays, teamGroupLevel], () => {
  router.replace({
    query: {
      ...(repository.value ? { repository: repository.value } : {}),
      ...(teamName.value ? { team: teamName.value } : {}),
      ...(teamFromTimestamp.value ? { from: teamFromTimestamp.value } : {}),
      days: teamLastNDays.value,
      group_level: teamGroupLevel.value === null ? 'null' : String(teamGroupLevel.value)
    }
  })
}, { flush: 'post' })

const isTeamFilesFlat = computed(() => {
  if (!teamFilesData.value) return false
  return !isFileNode(teamFilesData.value)
})

const teamFilesEntries = computed(() => {
  if (!isTeamFilesFlat.value || !teamFilesData.value) return []
  const data = teamFilesData.value
  if (isFileNode(data)) return []
  if (Array.isArray(data)) return data as Array<[string, number]>
  return Object.entries(data as Record<string, number>)
})

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
  if (isTeamFilesFlat.value || !teamFilesData.value) return null
  return teamFilesData.value as FileNode
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
  const fromIsoDate = toIsoDate(teamFromTimestamp.value)
  if (teamFromTimestamp.value && !fromIsoDate) {
    error.value = 'Invalid from date format. Use dd/mm/yyyy'
    return
  }

  loading.value = true
  error.value = null
  jsonExpandDepth.value = 0
  jsonViewerKey.value++
  try {
    teamFilesData.value = await getFilesModifiedByTeam({
      repository: repository.value,
      team_name: teamName.value,
      from_timestamp: fromIsoDate || undefined,
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
          <b-form-input v-model="teamName" type="text" placeholder="Enter team name" @keyup.enter="fetchTeamFiles" />
        </div>
        <div class="col-md-2">
          <label class="form-label">From Date (optional)</label>
          <b-form-input v-model="teamFromTimestamp" type="date" />
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
      <!-- Flat list (bar chart) -->
      <template v-if="isTeamFilesFlat">
        <bar-chart-component
          :data="teamFilesChartData || { labels: [], datasets: [] }"
          :options="chartOptions"
          :height="400"
          :horizontal="true"
        />
        <div class="mt-3">
          <h6>File Details:</h6>
          <ul class="file-list">
            <li v-for="([file, count], idx) in teamFilesEntries" :key="idx">
              <strong>{{ file }}</strong> — {{ count }} modifications
            </li>
          </ul>
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
          <treemap-chart-component :data="(teamFilesTreeData || {})" :height="500" />
          <div class="mt-3 tree-summary">
            <p><strong>Root:</strong> {{ (teamFilesData as FileNode).name }}</p>
            <p><strong>Total Modifications:</strong> {{ (teamFilesData as FileNode).modifications }}</p>
            <p class="mb-0"><strong>Direct Children:</strong> {{ (teamFilesData as FileNode).children?.length || 0 }}</p>
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
.file-list { list-style: none; padding: 0; max-height: 300px; overflow-y: auto; }
.file-list li { padding: 0.5rem; border-bottom: 1px solid #e9ecef; }
.file-list li:last-child { border-bottom: none; }
.tree-summary { padding: 1rem; background: #f8f9fa; border-radius: 0.5rem; margin-top: 1rem; }
.json-view { max-height: 600px; overflow: auto; border-radius: 0.5rem; }
.json-view :deep(.jv-container) { font-size: 0.875rem; }
.json-view :deep(.jv-code) { padding: 1rem; }
</style>
