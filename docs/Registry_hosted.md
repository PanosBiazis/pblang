# Registry hosted

This document describes how `sapphire` interacts with a **hosted** (HTTP/HTTPS)
registry, including authentication, token refresh, and publish retry behaviour.

---

## M4-gamma: Token refresh flow

When `sapphire publish` receives a 401 Unauthorized response from a hosted registry:

1. The CLI reads `PBLANG_REGISTRY_REFRESH_TOKEN` environment variable.
2. If set, posts a refresh request to `<registry>/auth/refresh`:
   ```
   POST <registry>/auth/refresh
   Content-Type: application/json
   {"refresh_token": "<PBLANG_REGISTRY_REFRESH_TOKEN>"}
   ```
3. On success, the response should contain `{"token": "<new_access_token>"}`.
4. The publish is retried once with the new token as Bearer auth.
5. If the retry succeeds, prints `Refreshed token and retried`.
6. If the retry also fails (non-2xx), or if no refresh token is set, the original
   401 error is returned.

### Environment variables

| Variable | Purpose |
|---|---|
| `PBLANG_REGISTRY_TOKEN` | Primary access token for publish/search (Bearer auth). |
| `PBLANG_REGISTRY_REFRESH_TOKEN` | Refresh token used to obtain a new access token on 401. |
| `PBLANG_REGISTRY_INDEX` | Registry base URL (e.g. `https://registry.pblang.io` or `file:///path/to/local/registry`). |

### Example publish flow with token refresh

```bash
export PBLANG_REGISTRY_INDEX=https://registry.example.com
export PBLANG_REGISTRY_TOKEN=my-access-token
export PBLANG_REGISTRY_REFRESH_TOKEN=my-refresh-token

# If the access token expires mid-session, publish auto-refreshes:
pblang publish
# Output: Refreshed token and retried
# Output: Published to https://registry.example.com
```

### Refresh endpoint contract

The hosted registry **must** implement:

```
POST /auth/refresh
Content-Type: application/json

Request body:
  {"refresh_token": "<opaque string>"}

Success response (HTTP 200):
  {"token": "<new_access_token>"}

Error response (HTTP 4xx/5xx):
  Any non-2xx status — publish_package surfaces the error as-is.
```

If the refresh endpoint itself returns a non-2xx status the error is propagated
and the package is **not** published.

### Code location

The implementation lives in `crates/pblang-pm/src/lib.rs`:

- **`publish_package`** — main publish function; matches on `ureq::Error::Status(401, _)`
  and delegates to `refresh_access_token`.
- **`refresh_access_token`** — private helper that POSTs to `/auth/refresh` and
  extracts the new token from the JSON response.
