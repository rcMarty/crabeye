// API service for Pull Request endpoints
const API_BASE_URL = import.meta.env.VITE_API_BASE_URL || 'http://localhost:8080'

// Base types
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
  state: PullRequestStatus
  events_history?: IssueEvent[] | null
  labels_history?: IssueLabel[] | null
}

// Response Types
export interface TopFilesResponse {
  repository: string
  pr_id: number
  file_path: string
  pr_creator: Contributor
}

// Request Parameter Interfaces
export interface ReviewersParams {
  repository: string
  file: string
  from_date?: string | null
  last_n_days?: number | null
  pagination?: Pagination | null
}

export interface TopFilesParams {
  repository: string
  name: string
  top_n: number
  duration?: number | null
}

export interface PrsInStateParams {
  repository: string
  state: PullRequestStatusType
  timestamp?: string | null
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

// Files Modified by Team Types
export type GroupingLevel = null | number | 'all' | 'none'

export interface FileNode {
  name: string
  modifications: number
  children: FileNode[]
}

export type FilesModifiedResponse = Array<[string, number]> | FileNode

export interface FilesModifiedByTeamParams {
  repository: string
  team_name: string
  from_timestamp?: string | null
  last_n_days?: number | null
  group_level?: GroupingLevel
}

/**
 * Get users who made reviews on a specific file within a date range
 */
export async function getReviewers(params: ReviewersParams): Promise<PaginatedResponse<Contributor>> {
  const searchParams = new URLSearchParams()
  searchParams.set('repository', params.repository)
  searchParams.set('file', params.file)

  if (params.from_date) {
    searchParams.set('from_date', params.from_date)
  }
  if (params.last_n_days != null) {
    searchParams.set('last_n_days', params.last_n_days.toString())
  }
  if (params.pagination) {
    searchParams.set('pagination[page]', params.pagination.page.toString())
    searchParams.set('pagination[per_page]', params.pagination.per_page.toString())
  }

  const response = await fetch(`${API_BASE_URL}/pr/reviewers?${searchParams.toString()}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch reviewers: ${response.statusText}`)
  }
  return response.json()
}

/**
 * Get top N files modified by a user within a duration
 */
export async function getTopFiles(params: TopFilesParams): Promise<TopFilesResponse[]> {
  const searchParams = new URLSearchParams()
  searchParams.set('repository', params.repository)
  searchParams.set('name', params.name)
  searchParams.set('top_n', params.top_n.toString())

  if (params.duration != null) {
    searchParams.set('duration', params.duration.toString())
  }

  const response = await fetch(`${API_BASE_URL}/pr/top-n-files?${searchParams.toString()}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch top files: ${response.statusText}`)
  }

  return response.json()
}

/**
 * Get count of PRs in a specific state at a given timestamp
 */
export async function getPrsInState(params: PrsInStateParams): Promise<number> {
  const searchParams = new URLSearchParams()
  searchParams.set('repository', params.repository)
  searchParams.set('state', params.state)

  if (params.timestamp) {
    searchParams.set('timestamp', params.timestamp)
  }

  const response = await fetch(`${API_BASE_URL}/pr/prs-in-state?${searchParams.toString()}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch PRs in state: ${response.statusText}`)
  }
  return response.json()
}

/**
 * Get the state and history of a PR at a specific timestamp
 */
export async function getPrHistory(params: PrHistoryParams): Promise<PrEvent> {
  const searchParams = new URLSearchParams()
  searchParams.set('repository', params.repository)
  searchParams.set('issue', params.issue.toString())
  searchParams.set('timestamp', params.timestamp)

  const response = await fetch(`${API_BASE_URL}/pr/pr-history?${searchParams.toString()}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch PR history: ${response.statusText}`)
  }

  return response.json()
}

/**
 * Get PRs that are currently waiting for review
 */
export async function getWaitingForReview(params: WaitingForReviewParams): Promise<PaginatedResponse<PrEvent>> {
  const searchParams = new URLSearchParams()
  searchParams.set('repository', params.repository)

  if (params.pagination) {
    searchParams.set('pagination[page]', params.pagination.page.toString())
    searchParams.set('pagination[per_page]', params.pagination.per_page.toString())
  }

  const response = await fetch(`${API_BASE_URL}/pr/waiting-for-review?${searchParams.toString()}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch waiting PRs: ${response.statusText}`)
  }
  return response.json()
}

/**
 * Get files modified by a team within a time window
 */
export async function getFilesModifiedByTeam(params: FilesModifiedByTeamParams): Promise<FilesModifiedResponse> {
  const searchParams = new URLSearchParams()
  searchParams.set('repository', params.repository)
  searchParams.set('team_name', params.team_name)

  if (params.from_timestamp) {
    searchParams.set('from_timestamp', params.from_timestamp)
  }
  if (params.last_n_days != null) {
    searchParams.set('last_n_days', params.last_n_days.toString())
  }
  if (params.group_level !== undefined && params.group_level !== null) {
    if (params.group_level === 'none' || params.group_level === 'all') {
      searchParams.set('group_level', params.group_level)
    } else {
      searchParams.set('group_level', params.group_level.toString())
    }
  }

  const response = await fetch(`${API_BASE_URL}/pr/files-modified-by-team?${searchParams.toString()}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch files modified by team: ${response.statusText}`)
  }
  return response.json()
}

/**
 * Helper function to extract status type from PullRequestStatus object
 */
export function getStatusType(status: PullRequestStatus): PullRequestStatusType {
  if ('WaitingForReview' in status) return 'WaitingForReview'
  if ('WaitingForBors' in status) return 'WaitingForBors'
  if ('WaitingForAuthor' in status) return 'WaitingForAuthor'
  if ('Open' in status) return 'Open'
  if ('Closed' in status) return 'Closed'
  if ('Merged' in status) return 'Merged'
  return 'Open' // fallback
}

/**
 * Helper function to get time from PullRequestStatus object
 */
export function getStatusTime(status: PullRequestStatus): string {
  if ('WaitingForReview' in status) return status.WaitingForReview.time
  if ('WaitingForBors' in status) return status.WaitingForBors.time
  if ('WaitingForAuthor' in status) return status.WaitingForAuthor.time
  if ('Open' in status) return status.Open.time
  if ('Closed' in status) return status.Closed.time
  if ('Merged' in status) return status.Merged.time
  return ''
}

/**
 * Get the state history of an issue at a specific timestamp
 * GET /api/issue/issue-events
 */
export async function getIssueEvents(params: IssueEventsParams): Promise<PullRequestStatus[]> {
  const searchParams = new URLSearchParams()
  searchParams.set('repository', params.repository)
  searchParams.set('issue', params.issue.toString())
  searchParams.set('timestamp', params.timestamp)

  const response = await fetch(`${API_BASE_URL}/issue/issue-events?${searchParams.toString()}`)
  if (!response.ok) {
    throw new Error(`Failed to fetch issue events: ${response.statusText}`)
  }
  return response.json()
}
