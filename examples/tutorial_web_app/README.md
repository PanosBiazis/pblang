# tutorial_web_app — Full-Stack Web App Demo

A complete HTTP server built with pblang, demonstrating routing, middleware,
JSON APIs, parameterized URLs, and static file serving.

## Features Demonstrated

| Feature          | How it's used                                    |
|------------------|---------------------------------------------------|
| **import**       | `http_server.sapp`, `json.sapp`, `fs.sapp`       |
| **functions**    | Route handlers, HTML generation, error wrapping   |
| **maps**         | In-memory database, JSON responses, route config  |
| **arrays**       | User records stored as array of maps              |
| **error handling** | `try/catch` wrapper around app handler           |
| **middleware**   | Request logging (`mw_log()`)                      |
| **routing**      | Exact routes + parameterized (`/api/users/:id`)   |
| **JSON API**     | `res_json()` for structured responses             |
| **HTML responses** | Hand-crafted HTML pages with inline CSS         |

## Setup & Run

```bash
# From the pblang project root:
pblang run examples/tutorial_web_app/main.sapp
```

## Endpoints

| Method | Path               | Description                     |
|--------|--------------------|---------------------------------|
| GET    | `/`                | HTML page with user directory   |
| GET    | `/api/users`       | JSON array of all users         |
| GET    | `/api/users/:id`   | JSON object for a single user   |
| GET    | `/api/stats`       | JSON aggregate statistics       |
| GET    | `/health`          | JSON health check               |

## Example Requests

```bash
# Get all users
curl http://127.0.0.1:8080/api/users

# Get user by ID
curl http://127.0.0.1:8080/api/users/2

# Get stats
curl http://127.0.0.1:8080/api/stats

# Health check
curl http://127.0.0.1:8080/health

# Homepage (HTML)
curl http://127.0.0.1:8080/
```

## Error Handling

The server wraps all route handling in `try/catch` — if any handler throws,
the client gets a 500 error page instead of a server crash.
