# Frontend — README

> Quick guide to run and build the frontend. Ensure the backend is running as a pre\-step.

## Prerequisites

- Node.js (>= 16) and `npm`.
- The backend must be running before starting the frontend.
  See [backend README](../backend/readme.md) for backend
  setup and how to run migrations and start the server.

## Setup

1. Change to the frontend folder:

```bash
  cd frontend
```

2. Install dependencies:

```bash
   npm install
```

## Start the server

```bash
  npm run serve
```

Visit the URL printed by the command (commonly `http://localhost:8080`).

[//]: # (TODO: update if needed)

[//]: # (## Environment / API URL)

[//]: # (- Create a local env file to point the frontend to the running backend. Example for Vite:)

[//]: # (    - Create `/.env.local` in `frontend` with:)

[//]: # (      ```)

[//]: # (      VITE_API_URL=http://localhost:8000)

[//]: # (      ```)

[//]: # (- For Vue CLI use:)

[//]: # (  ``VUE_APP_API_URL=http://localhost:8000``)

[//]: # (  )