// API service for all backend endpoints — matches the OpenAPI schema
const API_BASE_URL = import.meta.env.VITE_BACKEND_API_BASE_URL || 'http://localhost:7878/api'

/** Default repository constant */
export const DEFAULT_REPOSITORY = 'rust-lang/rust'

// ─── Base types ──────────────────────────────────────────────────────
export interface Contributor {
  github_id: number
  github_name: string
  name?: string | null
}

export interface Pagination {
  page: number
  per_page: number
}

export interface PaginatedResponse<T> {
  items: T[]
  page: number
  per_page: number
  total_count: number
}

export interface DateCount {
  date: string
  count: number
}

// Pull Request Status Types
export type PullRequestStatusType = 'WaitingForReview' | 'WaitingForBors' | 'WaitingForAuthor' | 'Open' | 'Closed' | 'Merged'

export interface PullRequestStatusWithTime {
  time: string
}

export interface PullRequestStatusMerged {
  merge_sha: string
  time: string
}

export type PullRequestStatus =
  | { WaitingForReview: PullRequestStatusWithTime }
  | { WaitingForBors: PullRequestStatusWithTime }
  | { WaitingForAuthor: PullRequestStatusWithTime }
  | { Open: PullRequestStatusWithTime }
  | { Closed: PullRequestStatusWithTime }
  | { Merged: PullRequestStatusMerged }

// Issue and PR Event Types
export interface IssueEvent {
  event: string
  timestamp: string
}

export interface IssueLabel {
  label: string
  timestamp: string
  action: 'ADDED' | 'REMOVED'
}

export interface PrEvent {
  repository: string
  pr_number: number
  author_id: number
  created_at: string
  state: PullRequestStatus
  events_history?: IssueEvent[] | null
  labels_history?: IssueLabel[] | null
}

// ─── Response Types ──────────────────────────────────────────────────
export interface TopFilesResponse {
  repository: string
  pr_id: number
  file_path: string
  pr_creator: Contributor
}

export interface FileNode {
  name: string
  modifications: number
  children: FileNode[]
}

/** Tagged union returned by /files-modified-by-team */
export type FilesModifiedResponse =
  | { type: 'list'; data: Record<string, number> }
  | { type: 'tree'; data: FileNode }

/** Response for a single PR count in a specific state */
export interface PrCountResponse {
  count: number
  to: string
  since?: string | null
}

/** Response for PR count over time (time-series) */
export interface PrCountOverTimeResponse {
  data: DateCount[]
  to: string
  since?: string | null
}

export type GroupingLevel = null | number | 'all' | 'none'

// ─── Request Parameter Interfaces ────────────────────────────────────
export interface ReviewersParams {
  repository: string
  file: string
  anchor_date?: string | null
  last_n_days?: number | null
  pagination?: Pagination | null
}

export interface TopFilesParams {
  repository: string
  name: string
  top_n: number
  last_n_days?: number | null
}

export interface PrsInStateParams {
  repository: string
  state: PullRequestStatusType
  anchor_date?: string | null
}

export interface PrsInStateOverTimeParams {
  repository: string
  state: PullRequestStatusType
  anchor_date?: string | null
  last_n_days?: number | null
}

export interface PrHistoryParams {
  repository: string
  issue: number
  timestamp: string
}

export interface WaitingForReviewParams {
  repository: string
  pagination?: Pagination | null
}

export interface IssueEventsParams {
  repository: string
  issue: number
  timestamp: string
}

export interface FilesModifiedByTeamParams {
  repository: string
  team_name: string
  anchor_date?: string | null
  last_n_days?: number | null
  group_level?: GroupingLevel
}

// ─── API Functions ───────────────────────────────────────────────────

function addPagination(sp: URLSearchParams, pagination?: Pagination | null) {
  if (pagination) {
    sp.set('page', pagination.page.toString())
    sp.set('per_page', pagination.per_page.toString())
  }
}

/**
 * GET /api/pr/reviewers
 * Get users who made reviews on a specific file/path within a date range
 */
export async function getReviewers(params: ReviewersParams): Promise<PaginatedResponse<Contributor>> {
  const sp = new URLSearchParams()
  sp.set('repository', params.repository)
  sp.set('file', params.file)
  if (params.anchor_date) sp.set('anchor_date', params.anchor_date)
  if (params.last_n_days != null) sp.set('last_n_days', params.last_n_days.toString())
  addPagination(sp, params.pagination)

  const response = await fetch(`${API_BASE_URL}/pr/reviewers?${sp}`)
  if (!response.ok) throw new Error(`Failed to fetch reviewers: ${response.statusText}`)
  return response.json()
}

/**
 * GET /api/pr/top-n-files
 * Get the N most recent file touches by a user within a time window
 */
export async function getTopFiles(params: TopFilesParams): Promise<TopFilesResponse[]> {
  const sp = new URLSearchParams()
  sp.set('repository', params.repository)
  sp.set('name', params.name)
  sp.set('top_n', params.top_n.toString())
  if (params.last_n_days != null) sp.set('last_n_days', params.last_n_days.toString())

  const response = await fetch(`${API_BASE_URL}/pr/top-n-files?${sp}`)
  if (!response.ok) throw new Error(`Failed to fetch top files: ${response.statusText}`)
  return response.json()
}

/**
 * GET /api/pr/prs-in-state
 * Get count of PRs in a specific state at a given timestamp
 */
export async function getPrsInState(params: PrsInStateParams): Promise<PrCountResponse> {
  const sp = new URLSearchParams()
  sp.set('repository', params.repository)
  sp.set('state', params.state)
  if (params.anchor_date) sp.set('anchor_date', params.anchor_date)

  const response = await fetch(`${API_BASE_URL}/pr/prs-in-state?${sp}`)
  if (!response.ok) throw new Error(`Failed to fetch PRs in state: ${response.statusText}`)
  return response.json()
}

/**
 * GET /api/pr/prs-in-state-over-time
 * Get count of PRs in a specific state for each day in a lookback window (time-series)
 */
export async function getPrsInStateOverTime(params: PrsInStateOverTimeParams): Promise<PrCountOverTimeResponse> {
  const sp = new URLSearchParams()
  sp.set('repository', params.repository)
  sp.set('state', params.state)
  if (params.anchor_date) sp.set('anchor_date', params.anchor_date)
  if (params.last_n_days != null) sp.set('last_n_days', params.last_n_days.toString())

  const response = await fetch(`${API_BASE_URL}/pr/prs-in-state-over-time?${sp}`)
  if (!response.ok) throw new Error(`Failed to fetch PR state time-series: ${response.statusText}`)
  return response.json()
}

/**
 * GET /api/pr/pr-history/{issue}
 * Get the states and labels of a PR at a specific timestamp (issue is a PATH param)
 */
export async function getPrHistory(params: PrHistoryParams): Promise<PrEvent> {
  const sp = new URLSearchParams()
  sp.set('repository', params.repository)
  sp.set('timestamp', params.timestamp)

  const response = await fetch(`${API_BASE_URL}/pr/pr-history/${params.issue}?${sp}`)
  if (!response.ok) throw new Error(`Failed to fetch PR history: ${response.statusText}`)
  return response.json()
}

/**
 * GET /api/pr/waiting-for-review
 * Get PRs that are currently waiting for review
 */
export async function getWaitingForReview(params: WaitingForReviewParams): Promise<PaginatedResponse<PrEvent>> {
  const sp = new URLSearchParams()
  sp.set('repository', params.repository)
  addPagination(sp, params.pagination)

  const response = await fetch(`${API_BASE_URL}/pr/waiting-for-review?${sp}`)
  if (!response.ok) throw new Error(`Failed to fetch waiting PRs: ${response.statusText}`)
  return response.json()
}

/**
 * GET /api/pr/files-modified-by-team
 * Get files modified by a team within a time window
 */
export async function getFilesModifiedByTeam(params: FilesModifiedByTeamParams): Promise<FilesModifiedResponse> {
  const sp = new URLSearchParams()
  sp.set('repository', params.repository)
  sp.set('team_name', params.team_name)
  if (params.anchor_date) sp.set('anchor_date', params.anchor_date)
  if (params.last_n_days != null) sp.set('last_n_days', params.last_n_days.toString())
  if (params.group_level !== undefined && params.group_level !== null) {
    if (params.group_level === 'none' || params.group_level === 'all') {
      sp.set('group_level', params.group_level)
    } else {
      sp.set('group_level', params.group_level.toString())
    }
  }

  const response = await fetch(`${API_BASE_URL}/pr/files-modified-by-team?${sp}`)
  if (!response.ok) throw new Error(`Failed to fetch files modified by team: ${response.statusText}`)
  return response.json()
}

/**
 * GET /api/issue/issue-events/{issue}
 * Get the state of an Issue at a specific timestamp (issue is a PATH param)
 */
export async function getIssueEvents(params: IssueEventsParams): Promise<IssueEvent[]> {
  const sp = new URLSearchParams()
  sp.set('repository', params.repository)
  sp.set('timestamp', params.timestamp)

  const response = await fetch(`${API_BASE_URL}/issue/issue-events/${params.issue}?${sp}`)
  if (!response.ok) throw new Error(`Failed to fetch issue history: ${response.statusText}`)
  return response.json()
}

/**
 * GET /api/teams
 * List all known teams
 */
export async function getTeams(): Promise<string[]> {
  const response = await fetch(`${API_BASE_URL}/teams`)
  if (!response.ok) throw new Error(`Failed to fetch teams: ${response.statusText}`)
  return response.json()
}

// ─── Helpers ─────────────────────────────────────────────────────────

/** Extract status type string from PullRequestStatus tagged union */
export function getStatusType(status: PullRequestStatus): PullRequestStatusType {
  if ('WaitingForReview' in status) return 'WaitingForReview'
  if ('WaitingForBors' in status) return 'WaitingForBors'
  if ('WaitingForAuthor' in status) return 'WaitingForAuthor'
  if ('Open' in status) return 'Open'
  if ('Closed' in status) return 'Closed'
  if ('Merged' in status) return 'Merged'
  return 'Open'
}

/** Extract timestamp from PullRequestStatus tagged union */
export function getStatusTime(status: PullRequestStatus): string {
  if ('WaitingForReview' in status) return status.WaitingForReview.time
  if ('WaitingForBors' in status) return status.WaitingForBors.time
  if ('WaitingForAuthor' in status) return status.WaitingForAuthor.time
  if ('Open' in status) return status.Open.time
  if ('Closed' in status) return status.Closed.time
  if ('Merged' in status) return status.Merged.time
  return ''
}
