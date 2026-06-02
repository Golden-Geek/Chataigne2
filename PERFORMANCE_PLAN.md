1. Executive Intent
This document defines an absolute, non-negotiable performance profile for the golden_core real-time data execution path. Chataigne2 functions as a high-frequency (60Hz–120Hz+ continuous throughput), mission-critical show control runtime.

Your task as an AI agent is to refactor the specified targeted components to strictly enforce:

Zero Allocations on Ingestion Paths: No malloc/free, heap reallocations, or string duplications during runtime loop execution.

Microsecond-Scale Worker Coordination: Complete elimination of arbitrary thread timeout loop blocks (recv_timeout).

Telemetry Decoupling: Removal of dynamic JSON serialization loops from performance monitoring channels.

2. Phase 1: Zero-Allocation Refactor in src/module/common/received_values.rs
Problem Statement
The incoming value synchronization pipeline utilizes heap-allocated vectors and heap strings (path_segments: &[String], label: head.clone(), children: Vec::new()). Under heavy tracking input (e.g., OSC tracking markers, high-density sensor grids), this causes heap fragmentation and micro-stuttering.

Implementation Blueprint
Convert Path Typings to Avoid Allocations:

Introduce a zero-allocation string dependency to replace String in the path segment tracking logic. Use smol_str::SmolStr or compact_str::CompactStr for layout keys.

Update the signature of apply_received_value_payload and apply_received_value_batch to accept a slice of inline/stack strings rather than a slice of heap-allocated String types.

Eliminate Dynamic Vectors in Tree Planning:

Modify the internal structure definition of PlannedReceivedFolder:

Rust
// BEFORE
struct PlannedReceivedFolder {
    label: String,
    existing_id: Option<NodeId>,
    children: Vec<PlannedReceivedNode>,
}

// AFTER
struct PlannedReceivedFolder {
    label: smol_str::SmolStr,
    existing_id: Option<NodeId>,
    children: smallvec::SmallVec<[PlannedReceivedNode; 8]>, // Bounded stack space allocation
}
Guard Against Early Mutation Copying:

In plan_single_leaf_under_existing, modify the algorithm to inspect the internal variant profile (param_types_match) prior to executing mutations or value duplications. Only perform modifications if a real change or type discrepancy is encountered.

3. Phase 2: Microsecond-Scale I/O Thread Refactor in src/module/modules/protocol/osc/osc_runtime.rs
Problem Statement
The isolated protocol background worker implements an explicit sleep loop fallback via command_rx.recv_timeout(Duration::from_millis(5)). This inserts up to 5 milliseconds of thread wake-up scheduling jitter when coordinating incoming commands and outbound network frames. Concurrently, OscOutboundMessage triggers a heap allocation for address paths on every network packet.

Implementation Blueprint
Transition Loop Execution to Event-Driven Polling:

Replace the arbitrary timer loop sleep sequence inside worker_loop with a true OS-native notification pattern using mio or epoll blocks.

Bind the non-blocking UdpSocket and an explicit cross-thread interrupt signal notifier (mio::Waker) into a unified mio::Poll loop instance. Let the host operating system thread scheduler park and wake the worker thread instantly upon either network frame arrival or command transmission events.

De-allocate Flight Telemetry Layouts:

Change OscOutboundMessage properties to stop using dynamic String values:

Rust
pub(crate) struct OscOutboundMessage {
    pub address: smol_str::SmolStr,       // Stack allocated if inline length matches
    pub payload: OscValuePayload,
    pub remote_address: std::net::SocketAddr, // Pre-resolved address representation
}
Remove the address query statement (resolve_socket_addr) from the runtime worker loop thread. Force resolution logic to execute on management threads prior to message serialization.

4. Phase 3: Telemetry Event Refactor in src/module/mod.rs
Problem Statement
The system indicators monitor live input and output traffic through an active dynamic macro serialization block: serde_json::json!({ "direction": direction }). This results in extensive runtime heap churn purely to trigger UI blinking dots.

Implementation Blueprint
Remove Dynamic Object Instantiation:

Delete the dynamic JSON allocation call inside the emit_traffic private utility method.

Inject Lock-Free Atomic Metrics Counters:

Add dedicated lock-free transaction counters directly to the core ModuleBase configuration struct:

Rust
pub struct ModuleBase {
    // ... core definitions
    pub incoming_traffic_count: std::sync::atomic::AtomicUsize,
    pub outgoing_traffic_count: std::sync::atomic::AtomicUsize,
}
Refactor Signaling Execution Paths:

Modify emit_incoming_traffic and emit_outgoing_traffic to drop tree event dispatch loops entirely. Replace them with direct .fetch_add(1, std::sync::atomic::Ordering::Relaxed) operations.

The decoupled frontend (src-ui) or state engine can poll these values on a controlled loop (e.g., 30Hz throttled intervals) without interrupting the engine's main execution loop.

5. Agent Constraints and Verification Guardrails
Enforce Validation Suite Checks: Execute cargo test immediately after modifying any single subsystem to guarantee no functional breakage of node structures.

Enforce Memory Allocation Guardrails: Do not introduce alternative allocation mechanisms (Box::new, BTreeMap, std::fmt::format!) into any tracking loops in received_values.rs or runtime worker loops in osc_runtime.rs.

Benchmark Validation: Use the optimization test suite under src/module/perf_tests.rs to measure and confirm performance improvements across revisions.