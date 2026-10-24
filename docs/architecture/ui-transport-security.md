# UI transport request boundary

Golden's transport host owns HTTP and WebSocket request admission. The engine, app shell and UI
stores do not decide which browser origins or HTTP authorities may reach project/script operations.

- `golden_transport_server::UiServerConfig` declares exact additional browser origins and Host
  authorities. Each list admits 32 entries, each at most 2,048 bytes. Invalid configuration fails
  startup; wildcard origins and authorities are unsupported.
- `hosts/transport/src/ui_server/access.rs` validates Host against the accepted socket's local
  address and port, then admits the same HTTP origin or an explicitly configured origin.
- `ui_server/http/request.rs` uses `httparse` for HTTP/1.1 syntax, with 128 header fields, the
  existing 32 KiB header limit and 16 MiB total request limit. Ambiguous singleton headers,
  transfer encoding and non-origin-form targets are rejected. The host closes each HTTP connection.
- `ui_server/http.rs` runs admission before API dispatch or WebSocket upgrade. HTTP responses
  echo only an admitted Origin and vary on Origin. Preflight permits the supported methods and
  headers. WebSocket handshakes use their own Origin admission and carry no wildcard CORS grant.
- Golden's desktop/headless launcher registers the origin of its selected development or explicit
  frontend URL. It does not grant every localhost port access.

Direct loopback, LAN IPv4 and IPv6 addresses remain usable without accounts or passwords. `localhost`
is accepted for loopback sockets. DNS names must appear as exact Host authorities, including the
listener port, in `allowed_hosts`; admission never resolves an arbitrary client-supplied hostname
and assumes that a matching DNS address establishes trust. The default launcher accepts this list
through `GC_UI_ALLOWED_HOSTS` as comma-separated authorities, for example `studio.local:7010`.

Native/headless clients use a valid Host authority and may omit Origin. Browser API requests with
cross-origin Fetch Metadata must supply an admitted Origin. Ordinary navigation to served assets
remains usable. This preserves the listener's trusted-network policy: a native peer can construct
its own headers, so browser-origin admission does not establish that peer's identity. The
[WebSocket protocol's origin model](https://www.rfc-editor.org/rfc/rfc6455.html#section-10.1)
describes that distinction.

## Sessions and privileged admission

`ui_server/session.rs` owns one registry per host. `POST /api/ui/session` bootstraps a credential
after Host/Origin admission using a JSON request of at most 1,024 bytes. A null capability request
uses `UiServerConfig::session_capabilities`; an explicit request must be a subset. The default
trusted listener grants the complete workbench capability set without accounts or passwords.

The registry admits 128 live sessions. Each has an opaque 256-bit OS-random token, an exact
Host/Origin binding and a one-hour lifetime. Renewal authenticates the existing token and retains
its slot; it cannot change privileges. Expired entries are reclaimed at bootstrap. Capacity returns
503 and does not evict a live client. Tokens from another host registry are invalid. Registry
teardown or `POST /api/ui/session/revoke` invalidates outstanding grants. Credentials and private
API responses use `Cache-Control: no-store`; credential debug output is redacted.

`http/routes.rs` owns the typed HTTP route set and its exhaustive capability selection. WebSocket
message capability selection is also exhaustive. Adding an operation therefore requires an
explicit privilege decision; unknown HTTP routes have no handler or effects.

Rust-owned `UiSessionCapabilities`, `UiSessionRequest` and `UiSessionResponse` in `golden_protocol`
generate the TypeScript contract. Capabilities are independent:

| Capability | Authorized surface |
| --- | --- |
| `observe` | Snapshots, subscriptions, replay and catalogs. |
| `author` | Graph edits, commands, history and edit-session cancellation/binding, including application side effects. |
| `project_files` | Direct project creation, upload, load and filesystem save operations. |
| `scripts` | Direct script-state, script-configuration and reload operations. |

Authoring is privileged and can invoke application I/O through ordinary node behavior. These
flags scope transport entrypoints; they do not sandbox authored graphs. Already trusted native
peers can obtain the host's configured grant. Public health, discovery and assets need no session.

HTTP uses an Authorization bearer header. Missing, expired, revoked or differently bound tokens
return 401 before dispatch; a missing privilege returns 403 before privileged dispatch or file
access. Snapshot edit cancellation additionally requires `author` before cancellation. WebSocket admission requires
observation privilege, and every operation checks its grant before reaching the hub. Renewal and
revocation checks use atomics outside the engine. Revocation/expiry closes an existing socket with
policy code 1008 and closes its application client lease. Already admitted work retains its normal
completion/cancellation contract; revocation does not undo completed effects.

The bootstrap response prepares the WebSocket protocol offers, including the credential offer.
Clients pass these offers unchanged; the server selects only `golden-ui`. Credentials stay out of
URLs and handshake response headers. Origin checks remain mandatory alongside the credential.

## Client ownership

`golden_ui/transport/session.ts` shares bootstrap and renewal between HTTP and WebSocket clients.
Concurrent callers share one pending bootstrap. An invalidation revision prevents a delayed
renewal from restoring rejected credentials. A socket opens only after bootstrap, with one opening
attempt shared across callers. Reconnect validates/renews its credential before opening again.
Observation-only clients omit authoring identity binding. A selected socket host also supplies
the default HTTP/bootstrap base, so LAN requests cannot fall back to localhost.
HTTP retries a rejected mutation only after a pre-dispatch 401; 403, 503, timeouts and network
failures are surfaced without replay. A host restart therefore refreshes credentials without
manufacturing a successful acknowledgement for an uncertain operation.

The Node 26 qualification helper imports this same credential implementation. Its bounded origin
cache supplies authenticated HTTP calls and the server-prepared WebSocket offers. Wire conversion
lives in focused `transport/codecs/{values,documents,projects}.ts`; `http.ts` owns request delivery,
and `ws-contracts.ts` owns socket bookkeeping types. UI stores continue to use transport interfaces.

CSP and the complete supported-surface qualification remain P05's final checkpoint in the
[current status](../progress/implementation-status.md). See the
[Tauri security reference](../reference/tauri-security.md) for native command permissions and CSP.
