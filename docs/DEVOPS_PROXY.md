# DevOps, proxy and service interoperability

Status: experimental PBLang-only libraries, with optional external integration.
This is a bounded initial slice, not all features of all DevOps products. It does
not install software, start listeners, open tunnels, join a VPN, deploy projects,
read accounts/credentials, create repositories or modify running services.

## Implemented now

| Area | Implemented | Not established |
| --- | --- | --- |
| DevOps pipelines | Existing workflow engine reused for deterministic dependencies; explicit callback execution; success/failure/skipped states | Shell/container runners, hard callback deadlines, artifacts, secrets, deployments or rollback |
| Proxy configuration | Typed loopback-only HTTP pools; Nginx and HAProxy text exporters; pure round-robin selection skipping caller-marked unhealthy backends | Native traffic relay, active health probes, TLS, DNS, HTTP/2/3, tunneling, WAF, cache or hot reload |
| ngrok | Read-only endpoint-list API recipe and opt-in caller-supplied transport interface | Bundled authenticated adapter, secure tunnel agent or service parity |
| Tailscale | Read-only tailnet-device API recipe and opt-in caller-supplied transport interface | VPN/WireGuard implementation, coordination, NAT traversal, DERP, ACL enforcement or device enrollment |
| Netlify | First-page site-list API recipe and opt-in caller-supplied transport interface | Hosted builds, CDN, deploy previews, functions, edge runtime or rollback |

These libraries can be imported by existing PBLang products. No product-specific
GUI/MCP command or foreign-language runtime adapter was added in this slice.
PBLang remains canonical; generated external configuration is interoperability,
not proof that an external binary executed or that platforms behave identically.

## Native pipeline

```sapp
let Ops=import("stdlib:devops_pipeline.sapp");
let jobs=[map{id:"build",needs:[]},map{id:"test",needs:["build"]}];
let order=Ops.order(jobs); // no callbacks
let report=Ops.run(jobs,fn(id){
    // Replace this benign callback with your own trusted PBLang operation.
    return map{ok:true};
},true);
```

The entire graph is validated before effects: 1–32 jobs, at most 16 distinct
dependencies each, exact `{id,needs}` schema, identifiers of 1–48 ASCII letters,
digits, underscores or hyphens, known dependencies and no cycles. Inputs are
snapshotted so callbacks cannot change the prepared dependency graph. Execution
is synchronous and opt-in. A callback receives its job id and must return exactly
`{ok:bool}`; exceptions and malformed responses fail that job without exposing
its output. Dependent jobs are skipped; independent jobs continue. Each job is
called at most once. There is no retry, rollback or time/memory sandbox around
trusted callback code. `hard_timeout_enforced` and `rollback_performed` are false.
Job identifiers are returned to the caller and must not contain private content.

## Proxy configuration and selection

```sapp
let Proxy=import("stdlib:proxy_config.sapp");
let config=Proxy.create(8080,[map{id:"app_a",port:8081},map{id:"app_b",port:8082}]);
let nginx_text=Proxy.nginx(config);
let haproxy_text=Proxy.haproxy(config);
let choice=Proxy.select(config,[true,false],0);
```

Configuration has exactly `listen_port`, `upstreams`, `connect_timeout_ms` and
`io_timeout_ms`. All ports are integers 1024–65535; timeouts are 1–60000 ms.
There are 1–16 distinct upstreams, each exactly `{id,port}`; lowercase ASCII
letters, digits and underscores only in 1–32 character ids. Duplicate ports/ids
and a backend matching the listener port are rejected. Every address is the
literal loopback address, not caller-supplied DNS or an arbitrary network target.
No raw directives, shell fragments, TLS keys or credentials are accepted.

Selection requires one exact boolean health value per backend and a bounded
cursor. It returns a copied backend plus the next cursor, or `available:false`
with no fallback when all are unhealthy. This does not probe a host or certify
health freshness. Exported proxy behavior is separate from this pure selector.

Exporters return text only. They are NOT configuration loaders. They differ:
Nginx disables request-header forwarding except its fixed Host/Connection and
Content-Type settings, limits request bodies to 64 KiB and disables retry and
buffering. HAProxy retains its ordinary HTTP header/body behavior. They are not
security-equivalent policies or transparent substitutes. Both configure basic
timeouts, round-robin pools and a loopback listener, without access logging.
Timeout directives are not a proof of an end-to-end wall-clock deadline.
Both outputs require review and syntax validation with the operator's actual
binary before activation; neither binary was installed/started for these tests.

## Optional service communication

`stdlib:ops_services.sapp` provides `request(service,params)` and
`fetch(service,params,transport,allow_transport=false)`. Supported recipes:

- `ngrok`, no params: `GET https://api.ngrok.com/endpoints`, API version 2.
- `tailscale`, `{tailnet:"example.com"}`: GET the encoded tailnet's device list.
- `netlify`, no params: GET sites page 1, at most 100 entries requested.

Origin, method and headers come from fixed recipes, not arbitrary caller URLs.
Unknown parameters, controls, path separators and single/double-dot path segments fail
closed. Plans request a 5-second deadline, 65536 response bytes, no redirects and
no retries. These are instructions to the adapter, not VM-enforced limits.

```sapp
let API=import("stdlib:ops_services.sapp");
let plan=API.request("tailscale",map{tailnet:"example.com"});
// Demonstration only: replace with a reviewed authenticated HTTPS transport.
let fixture_transport=fn(request){return map{status:200,body:"{\"devices\":[]}"};};
let result=API.fetch("tailscale",map{tailnet:"example.com"},fixture_transport,true);
```

`fetch` invokes the supplied trusted callback once only after explicit opt-in.
Authentication must be bound to the selected service out-of-band by that adapter.
Never send one service's token to another, use insecure TLS, follow redirects,
log plans/results, or pass credentials in argv or temporary files. Successful
results intentionally return private API data to the calling application and are
marked `safe_to_log:false`. Error responses and callback exceptions are replaced
with fixed messages. The exact adapter result is `{status:number,body:string}`;
2xx JSON is parsed with duplicate-key rejection, depth 12, 4096 items and a
65536-byte returned-body limit. Invalid or oversized responses have no data.
No auto-pagination, retries, mutation methods or response-driven follow-up URLs
exist. `complete_inventory`, `transport_verified` and `hard_timeout_enforced`
remain false even on success. A callback can allocate or block before returning;
the library is not a hostile-callback containment boundary.

### Why a bundled authenticated transport is still blocked

Static review found that `http_client.sapp` stores but does not apply its header
and timeout settings in get/post. Its permissive parser is unsuitable for a
general reverse proxy. The legacy request builder and response helper can also
produce ambiguous framing when used for untrusted forwarding; they are not used
by these new modules. A safe relay still needs strict HTTP framing, peer/origin
binding, TLS, cancellation and independently reaped workers.

The alternative supervised curl route also needs work: `linux_process.sapp`
writes `stdin_bytes` to a named temporary file. Passing a token through that
input would still persist it, even though normal cleanup removes the file. No
authenticated curl adapter was added. Confidential anonymous input, platform
verification and explicit trust/authentication handling remain prerequisites.

## Verification and remaining roadmap

Run the offline composition example:

```sh
pblang run examples/devops_proxy_workbench.sapp
```

Focused synthetic tests: `devops_pipeline`, `proxy_config`, `ops_services`, and
the existing `workflow` regression. They exercise real PBLang callback execution,
dependency failures/cycles/snapshots, config injection/duplicates, deterministic
selection, fixed API recipes, opt-in, masked errors and bounded strict JSON. No
external API, credential, proxy process or account is needed. Foreign config
syntax, live SaaS auth, native network transport and physical platforms are not
covered. This does not change language syntax or promise full production parity.

Next independent slices: confidential transport; strict HTTP proxy forwarding;
TLS/streaming/health probes and overload controls; explicit CI runners/artifacts
and reversible deployments; tunnel/mesh protocols and key lifecycle; hosting,
previews and function packaging. Each needs its own limits and positive/negative
integration evidence, not a declaration that an entire commercial service has
been reproduced.

### 2026-09-05 validation record

The four focused tests and offline composition example passed both in the
working checkout and in an isolated snapshot containing committed dependencies
plus this slice, using the existing retained PBLang executable. PBOffice's
headless Sheets editor regression also passed after its local dependency lock
was refreshed. No full suite, network listener, account query or deployment ran.
Source checking and lint completed without errors; retained lint still reports
unused-variable/style warnings. Independent read-only security review found no
material issue in the retained slice. The PBLang Graphify refresh timed out at
20 seconds and is not claimed current.

The commit's portable dependency lock describes the isolated snapshot, not
unrelated dirty workspace changes. The working checkout keeps its separate
matching local lock so existing unfinished work is preserved.

## Primary references used for scope and API recipes

- [NGINX proxy module](https://nginx.org/en/docs/http/ngx_http_proxy_module.html)
- [HAProxy health checks](https://www.haproxy.com/documentation/haproxy-configuration-tutorials/reliability/health-checks/)
- [GitLab pipeline concepts](https://docs.gitlab.com/ci/pipelines/)
- [ngrok endpoint-list API](https://ngrok.com/docs/api-reference/endpoints/list)
- [ngrok API version/auth](https://ngrok.com/docs/api/index)
- [Tailscale API](https://tailscale.com/docs/reference/tailscale-api)
- [Tailscale connection types](https://tailscale.com/docs/reference/connection-types)
- [Netlify API](https://docs.netlify.com/api-and-cli-guides/api-guides/get-started-with-api/)
- [Netlify deployment overview](https://docs.netlify.com/deploy/deploy-overview/)
