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

If resume fails, `ResumeError::into_parts` returns both the OS error and suspended owner. Keep that
owner through bounded termination/retirement observation; a failed resume is not cleanup proof.

Creation returns `CreateError` rather than discarding a process created before later verification
fails. `error()` exposes the original IO error/code. `into_parts()` returns that error and an optional
retirement-only `OwnedProcess`: None means no target was returned; Some must stay owned through
termination and verified retirement. It has no resume operation. The native event thread continues
observing the retained owner on either result path. Dropping the error requests termination but
does not prove cleanup. MapGyver keeps a returned owner in its existing stop/retirement state,
publishes its PID, and prevents retry/replacement or slot release before completion proof.
Validation/OS failures before a target exists retain ordinary failure/backoff behavior.

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

Windows launches with `DEBUG_PROCESS` on a dedicated native event thread. A creation event
suspends user execution until that thread verifies job membership and duplicates the supplied
process handle. It continues exit events before checking the duplicated handle for kernel
completion. It closes image/DLL file handles; Windows owns event process/thread handles.
The observer must finish successfully before retirement can pass. App callers may move the
owner between threads; only the private creating thread waits/continues debug events.
This uses the documented [creation/exit event contract](https://learn.microsoft.com/en-us/windows/win32/debug/debugging-events)
and [thread-affine wait API](https://learn.microsoft.com/en-us/windows/win32/api/debugapi/nf-debugapi-waitfordebugevent).

This is visible debugging, with startup/event overhead: targets can detect a debugger and
cannot simultaneously use another debugger for the same process. The initial first-chance
primary-thread [loader breakpoint](https://learn.microsoft.com/en-us/windows-hardware/drivers/debugger/initial-breakpoint)
is consumed, with a distinct one-time WOW64 handshake for processes identified by
`IsWow64Process`; subsequent breakpoints and application
exceptions are forwarded to Windows. Targets requiring a separate debugging chain, protected
targets or anti-debug behavior are not silently accommodated. A descendant that explicitly
starts a [new debugging chain](https://learn.microsoft.com/en-us/windows/win32/procthread/process-creation-flags)
can leave a job-accounting gap; that remains unverified, never successful retirement. No debug
privilege is enabled, pre-existing process attached, target memory inspected or code injected.

`request_close` posts nonblocking `WM_CLOSE` only to windows whose observed job identities remain
pinned. Its count measures requests, not acknowledgements; a headless target may have no windows.
An app can provide another graceful protocol, wait to its deadline, then call `terminate` for the
owned job and continue bounded retirement observation. Drop explicitly terminates the job without
joining the event thread or claiming cleanup completion; its retained job handle closes after its
final event. This also cancels a never-resumed primary thread.

Launch strings and explicit environment blocks admit at most 32,767 UTF-16 units, reject NULs and
duplicate case-insensitive environment names, and never concatenate shell syntax. Live debugging
identities and retained descendant handles are bounded at 4,096; excess/failed observation remains an
error or an accounting gap. Native creation can block inside Windows, so perform it on a background
owner and check app cancellation before resume. Non-Windows targets explicitly return Unsupported.
Sender readiness, singleton handoffs, elevated/launcher/breakaway behavior and app recovery still
require app-owned outcomes and qualification.

Run `cargo test --locked -p golden_io --all-features` for the existing IO suites and the finite
owned executable, hidden-window, descendant and cancellation checks. The `process-fixture` feature
builds only test fixtures; they do not launch arbitrary applications or display a window.
The explicitly ignored WOW64 system-tool case retains an unverified job lifetime; consult
`docs/progress/mapgyver-m08-retirement.md` before replay. Its two attempted runs exhausted the
current investigation allowance, and the last used a mixed-separator WHERE pattern (exit 2), so it
does not qualify ordinary 32-bit execution. The corrected fixture uses a plain filename pattern
with an explicit System32 working directory and has not run. Do not interpret
the x64 fixture cohort or default passing tests as complete WOW64 qualification.
