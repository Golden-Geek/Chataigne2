# Golden OSCQuery

The shared desktop and headless host adds **Preferences → OSCQuery**. The folder starts disabled. When enabled, its **Port** serves the current node hierarchy over HTTP and accepts OSC over UDP on the local machine. MapGyver defaults to 43000 and Chataigne to 42000. The port is saved with application Preferences. Edits rebind both sockets, and an occupied TCP or UDP port is retried while the preference remains enabled.

Enable **Allow LAN Control** to advertise the service via DNS-SD as `_oscjson._tcp.local.` and let devices on the computer's active IPv4 subnets browse and edit the app. This setting is saved with Preferences. The advertisement starts once both sockets are bound, uses the actual configured port and follows network interface changes. Changing the port replaces the advertisement. Turning off LAN Control withdraws it and returns the listener to the local machine. HTTP and WebSocket clients can use the resolved IPv4 address or the advertised `.local` hostname. [OSCQuery proposal](https://github.com/Vidvox/OSCQueryProposal)

`GET /?HOST_INFO` reports the application's name, supported attributes and OSC endpoint. Browse the entire hierarchy with `GET /`, request any child by `FULL_PATH`, or request individual attributes such as `?VALUE` or `?RANGE`. Each response uses one immutable Golden projection, so the address, metadata and values describe the same revision. The transport reads that projection off the engine thread. [OSCQuery proposal](https://github.com/Vidvox/OSCQueryProposal)

A segment uses the Golden node's short name when it is unique among its siblings and consists of ASCII letters, digits, underscores or hyphens. An invalid, empty, overlong or duplicate short name uses `~` followed by the node UUID without hyphens. This keeps every node independently addressable. The root is `/`. Boolean values use integer `0` and `1`, vectors and RGBA use float components, and reference values use the target UUID. Request a smaller subtree if a response exceeds the host's 32 MiB or 256-level limit.

Send OSC messages to the `FULL_PATH` of a writable parameter. A trigger accepts an empty message, and booleans accept `0`, `1`, or a native OSC boolean. A bare `ACCESS` value of 1 means read, 2 means write (for example a trigger), and 3 allows both. Incoming values and immediate bundles use the ordinary Golden edit, validation and history path. The local input queue bounds each datagram to 4 KiB and each bundle to 32 edits. The control actor rejects queued edits that belong to a replaced project. Time-tag scheduling is outside the current input implementation.

## Workbench commands

Connect a WebSocket to `/` on the `WS_PORT` reported by `HOST_INFO`. Send text messages with `COMMAND`, `DATA` and an optional string `ID`. Each edit returns `{"COMMAND":"RESULT","DATA":{"command":"…","ok":true}}` and echoes the `ID` when supplied. Failed edits return `ok: false` and an `error` message. `ADD` also returns the created item's address when it is still available.

| Command | `DATA` |
| --- | --- |
| `ADD` | `{address, type, name?}`. Query the manager's `GOLDEN_ADD_TYPES` for its catalog. Use a menu label or an unambiguous type. |
| `REMOVE` | `{address}`. Uses the node's remove permission. |
| `RENAME` | `{address, name}`. Uses the workbench's label edit. |
| `MOVE` | `{address, parent, before?}`. Provide the address of a direct child of the destination to insert before it. Omit `before` to append. |
| `PATCH_META` | `{address, meta}`. Query `?GOLDEN_META` for the current label, description, tags, enable state, custom color, collapse state and permissions. Send the fields to change. A `null` color or description clears its override. Colors use `{r,g,b,a}` with components between 0 and 1. |
| `UNDO`, `REDO` | No payload. Uses the shared document history. |

Creation, removal, ordering and metadata edits use normal Golden intents. If the project or target metadata changes between resolution and admission, the edit is rejected so a client can read the current address and state before retrying. The service closes established WebSockets when the port changes or the preference is disabled.

## Live feedback

Send `LISTEN` or `IGNORE` with an exact parameter address in `DATA`. Each connection has its own subscriptions. To request the current value when subscribing, use `[address, true]` or `{address, sendFeedback: true}`. Trigger subscriptions begin with the next pulse. The command result is text and feedback is binary OSC. [OSCQuery proposal](https://github.com/Vidvox/OSCQueryProposal) [Legacy OSC remote control](https://github.com/benkuper/juce_organicui/blob/master/remotecontrol/OSCRemoteControl.cpp)

Isolated committed updates are sent as soon as the read model publishes them. During a burst, the client receives immediate-timetag OSC bundles spaced at least 20 ms apart. A bundle contains the latest value of each changed control and retains trigger pulses in publication order. The worker observes only subscribed parameter events and resolves paths again when the graph changes. Removing or remapping a path retires that subscription. Project replacement closes the connection so a new client starts with the current project's addresses.

Each connection supports up to 128 subscriptions. Feedback frames are limited to 64 KiB and 512 messages. A client whose backlog exceeds the event window or bundle limit must reconnect and subscribe again.

## Workbench addresses

Right-click a node and choose **Copy OSC Address** to copy its current `FULL_PATH`. Enable **Preferences → Interface → Show OSC Address in Help** to see the path beside the hovered node's name. This preference starts off. Both actions ask the Golden host for the canonical path, including after a move or a duplicate short name. The path is available before enabling the OSCQuery listener.

The workbench lookup uses an authenticated observation request scoped to the originating project generation. The host walks the requested node's ancestor chain against a coherent projection and uses the same short-name and UUID rules as OSCQuery. The footer refreshes on hover, node path changes and project reload, without requesting addresses during ordinary value updates.
