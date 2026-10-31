# Owned process trees

`golden_io::owned_process` owns Windows process resources. App code owns executable definitions,
readiness, demand aggregation, graceful protocols, restart/backoff and project-generation fencing.
Use this API from an IO/background owner; it does not create an engine polling loop or block an
actor on process startup/shutdown.

`SuspendedProcess::create` accepts an absolute `.exe` path, CRT-style argument values, optional
absolute working directory, inherited or fully replaced environment, and hidden/normal window mode.
It creates a private non-inheritable kill-on-close Job Object, assigns it during suspended process
creation, verifies membership and returns ownership before resume. `resume` consumes that
suspended owner. Cancellation can terminate/wait while suspended or simply drop ownership. There is
no executable-name lookup, implicit shell, elevation fallback or ownership of pre-existing processes.

`ProcessSpec::validate` checks the same launch encoding and configuration bounds without creating
a process or opening the executable. Applications can reject invalid authored definitions before
committing them; successful validation does not prove the target exists or will start.

The implementation uses the existing `windows` bindings. The standard-library primary-thread and
process-attribute APIs are still unstable on the repository's Rust 1.97 toolchain. The reviewed
`win32job` wrapper covers job setup but not the complete creation/resume/retirement contract.
See [Rust's Windows process extensions](https://doc.rust-lang.org/stable/std/os/windows/process/trait.ChildExt.html),
[job-list assignment](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-updateprocthreadattribute)
and [Job Objects](https://learn.microsoft.com/en-us/windows/win32/procthread/job-objects).

`snapshot` distinguishes root exit, active job members, observed descendant handles still awaiting
completion, and lifetimes that were never captured by a handle. Root exit and zero active accounting
alone are not a complete-retirement barrier. `wait_tree(timeout)` returns true only after the
root and captured handles signal and accounting has no unobserved lifetimes. A missed transient
process leaves retirement unverified; false at the deadline is not success. Keep that distinction
in app observations rather than guessing that every target cleaned up. Holding verified handles
also prevents PID reuse from authorizing actions on another process.

`request_close` posts nonblocking `WM_CLOSE` only to windows whose observed job identities remain
pinned. Its count measures requests, not acknowledgements; a headless target may have no windows.
An app can provide another graceful protocol, wait to its deadline, then call `terminate` for the
owned job and continue bounded retirement observation. Drop requests kill-on-close without joining
the worker or claiming cleanup completion.

Launch strings and explicit environment blocks admit at most 32,767 UTF-16 units, reject NULs and
duplicate case-insensitive environment names, and never concatenate shell syntax. Member-list
queries and retained descendant handles are bounded at 4,096; excess/failed observation remains an
error or an accounting gap. Native creation can block inside Windows, so perform it on a background
owner and check app cancellation before resume. Non-Windows targets explicitly return Unsupported.
Sender readiness, singleton handoffs, elevated/launcher/breakaway behavior and app recovery still
require app-owned outcomes and qualification.

Run `cargo test --locked -p golden_io --all-features` for the existing IO suites and the finite
owned executable, hidden-window, descendant and cancellation checks. The `process-fixture` feature
builds only test fixtures; they do not launch arbitrary applications or display a window.
