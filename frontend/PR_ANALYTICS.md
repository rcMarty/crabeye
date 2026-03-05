# PR Analytics Graphs

This document describes the newly created PR analytics visualization features.

## Overview

The PR Analytics page provides interactive graphs and visualizations for Pull Request data using the following endpoints:

- `/api/pr/reviewers` - Get users who made reviews on specific files
- `/api/pr/top-n-files` - Get top N files modified by a user
- `/api/pr/prs-in-state` - Get count of PRs in specific states
- `/api/pr/pr-state` - Get PR state at specific timestamp
- `/api/pr/waiting-for-review` - Get PRs waiting for review

## New Files Created

### 1. API Service (`src/services/prApi.ts`)
Provides TypeScript functions to interact with all PR endpoints:

```typescript
// Get reviewers for a file
await getReviewers({ file: 'src/lib.rs', last_n_days: 30 })

// Get top files for a user
await getTopFiles({ user_id: 123, top_n: 10, duration: 30 })

// Get PRs in a specific state
await getPrsInState({ state: 'open', timestamp: '2026-02-03' })

// Get waiting PRs
await getWaitingForReview()
```

### 2. Bar Chart Component (`src/components/Charts/BarChartComponent.vue`)
Reusable Chart.js bar chart component with:
- Horizontal/vertical orientation support
- Responsive sizing
- Custom height configuration
- Auto-update on data changes

### 3. PR Analytics Page (`src/pages/PrAnalytics.vue`)
Main dashboard page with three interactive sections:

#### A. Top Modified Files Chart
- **Input**: User ID, number of files (Top N), duration in days
- **Visualization**: Horizontal bar chart
- **Features**: 
  - File name tooltips showing full paths
  - Detailed file list below chart
  - Color-coded bars

#### B. PR Status Distribution
- **Input**: Optional date filter (defaults to today)
- **Visualization**: Pie chart + statistics grid
- **Shows**: Count of Open, Closed, Merged, and Total PRs
- **Features**: Interactive stats cards with color-coded values

#### C. PRs Waiting for Review
- **Visualization**: Doughnut chart + PR number badges
- **Shows**: List of all PR numbers awaiting review
- **Features**: Clickable PR badges, scrollable list

## Usage

### Access the Page
Navigate to `/pr-analytics` in your application, or use the route name:

```javascript
this.$router.push({ name: 'PrAnalytics' })
```

### Configuration
Set your API base URL in environment variables:

```env
VITE_API_BASE_URL=http://localhost:8080
```

## Component API

### BarChartComponent Props
```typescript
{
  data: Object,      // Chart.js data object
  options: Object,   // Chart.js options (optional)
  height: Number,    // Height in pixels (default: 300)
  horizontal: Boolean // Horizontal bars (default: false)
}
```

## Example: Fetching Top Files

```vue
<script setup>
import { ref } from 'vue'
import { getTopFiles } from '@/services/prApi'

const topFiles = ref([])

async function loadData() {
  topFiles.value = await getTopFiles({
    user_id: 42,
    top_n: 10,
    duration: 30
  })
}
</script>
```

## Styling

The page uses Bootstrap Vue components and custom CSS for:
- Responsive grid layouts
- Card-based sections
- Color-coded statistics
- Scrollable lists
- Loading states and error handling

## Error Handling

All API calls include error handling with user-friendly error messages displayed in dismissible alerts.

## Future Enhancements

Potential improvements:
1. Add time-series line charts for PR trends over time
2. Add PR state timeline visualization using `/api/pr/pr-state`
3. Add reviewer contribution charts using `/api/pr/reviewers`
4. Add date range selectors for historical analysis
5. Export charts as images or PDFs
6. Add real-time updates with WebSocket
