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

Scoped session capabilities, client session recovery and CSP have their own P05 acceptance
checkpoints in the [current status](../progress/implementation-status.md). Origin/Host admission
alone does not close those checkpoints. See the [Tauri security reference](../reference/tauri-security.md)
for native command permissions and the CSP boundary.
