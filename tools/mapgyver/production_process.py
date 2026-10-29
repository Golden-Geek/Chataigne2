"""A Windows watchdog scoped to the renderer process created by this invocation."""

import ctypes
from ctypes import wintypes
import os
import subprocess
import time


class _BasicLimits(ctypes.Structure):
    _fields_ = [("process_time", ctypes.c_int64), ("job_time", ctypes.c_int64),
                ("flags", wintypes.DWORD), ("minimum_set", ctypes.c_size_t),
                ("maximum_set", ctypes.c_size_t), ("active_limit", wintypes.DWORD),
                ("affinity", ctypes.c_size_t), ("priority", wintypes.DWORD),
                ("scheduling", wintypes.DWORD)]


class _IoCounters(ctypes.Structure):
    _fields_ = [(name, ctypes.c_uint64) for name in
                ("read_ops", "write_ops", "other_ops", "read_bytes", "write_bytes", "other_bytes")]


class _ExtendedLimits(ctypes.Structure):
    _fields_ = [("basic", _BasicLimits), ("io", _IoCounters),
                ("process_memory", ctypes.c_size_t), ("job_memory", ctypes.c_size_t),
                ("peak_process_memory", ctypes.c_size_t), ("peak_job_memory", ctypes.c_size_t)]


class _Accounting(ctypes.Structure):
    _fields_ = [(name, ctypes.c_int64) for name in ("user", "kernel", "period_user", "period_kernel")]
    _fields_ += [(name, wintypes.DWORD) for name in ("faults", "processes", "active", "terminated")]


class _Job:
    def __init__(self):
        self.api = ctypes.WinDLL("kernel32", use_last_error=True)
        declarations = {
            "CreateJobObjectW": ([ctypes.c_void_p, wintypes.LPCWSTR], wintypes.HANDLE),
            "SetInformationJobObject": ([wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p, wintypes.DWORD], wintypes.BOOL),
            "AssignProcessToJobObject": ([wintypes.HANDLE, wintypes.HANDLE], wintypes.BOOL),
            "QueryInformationJobObject": ([wintypes.HANDLE, ctypes.c_int, ctypes.c_void_p,
                                           wintypes.DWORD, ctypes.c_void_p], wintypes.BOOL),
            "TerminateJobObject": ([wintypes.HANDLE, wintypes.UINT], wintypes.BOOL),
            "CloseHandle": ([wintypes.HANDLE], wintypes.BOOL),
            "QueryPerformanceCounter": ([ctypes.POINTER(ctypes.c_int64)], wintypes.BOOL),
            "QueryPerformanceFrequency": ([ctypes.POINTER(ctypes.c_int64)], wintypes.BOOL),
        }
        for name, (arguments, result) in declarations.items():
            function = getattr(self.api, name)
            function.argtypes, function.restype = arguments, result
        self.handle = self.api.CreateJobObjectW(None, None)
        if not self.handle:
            raise ctypes.WinError(ctypes.get_last_error())
        limits = _ExtendedLimits()
        limits.basic.flags = 0x2000  # JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE.
        if not self.api.SetInformationJobObject(self.handle, 9, ctypes.byref(limits), ctypes.sizeof(limits)):
            error = ctypes.get_last_error()
            self.close()
            raise ctypes.WinError(error)

    def assign(self, process):
        # Popen's retained Windows process handle fences identity independently of PID reuse.
        if not self.api.AssignProcessToJobObject(self.handle, wintypes.HANDLE(int(process._handle))):
            raise ctypes.WinError(ctypes.get_last_error())

    def active(self):
        value = _Accounting()
        if not self.api.QueryInformationJobObject(self.handle, 1, ctypes.byref(value), ctypes.sizeof(value), None):
            raise ctypes.WinError(ctypes.get_last_error())
        return value.active

    def stamp(self):
        coordinate, frequency = ctypes.c_int64(), ctypes.c_int64()
        if not self.api.QueryPerformanceCounter(ctypes.byref(coordinate)) \
                or not self.api.QueryPerformanceFrequency(ctypes.byref(frequency)):
            return None, None
        return coordinate.value, frequency.value

    def terminate(self):
        if not self.api.TerminateJobObject(self.handle, 1):
            raise ctypes.WinError(ctypes.get_last_error())

    def close(self):
        if self.handle:
            self.api.CloseHandle(self.handle)
            self.handle = None


def run_owned(command, timeout, stdout, stderr):
    """Never terminate by executable name or inspect unrelated jobs/processes."""
    if os.name != "nt":
        raise RuntimeError("native production capture requires Windows")
    job, process = _Job(), None
    started = time.monotonic()
    timed_out, forced = False, False
    try:
        process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                   creationflags=subprocess.CREATE_NO_WINDOW)
        job.assign(process)
        try:
            process.wait(timeout=timeout)
        except subprocess.TimeoutExpired:
            timed_out = forced = True
            job.terminate()
            process.wait(timeout=10)
        until = time.monotonic() + 5
        while job.active() and time.monotonic() < until:
            time.sleep(0.02)
        active = job.active()
        if active:
            forced = True
            job.terminate()
        coordinate, frequency = job.stamp()
        return {"exit_code": process.returncode, "timed_out": timed_out, "forced_cleanup": forced,
                "owned_job_active_before_close": active, "elapsed_seconds": time.monotonic() - started,
                "exit_observed_qpc": coordinate, "qpc_frequency": frequency,
                "job_assignment_atomic": False,
                "watchdog_scope": "one newly created renderer and its inherited job descendants"}
    finally:
        try:
            if process is not None and process.poll() is None:
                # Assignment can fail under restrictive parent jobs. Terminate only our retained
                # Popen handle; do not broaden cleanup to other processes or installed services.
                process.kill()
                process.wait(timeout=10)
        finally:
            job.close()
