import importlib.util
import ctypes
import io
from pathlib import Path
import subprocess
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location("production_process", ROOT / "tools/mapgyver/production_process.py")
PROCESS = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(PROCESS)


class Process:
    pid = 123
    def __init__(self, timeout=False):
        self.returncode = None
        self.timeout = timeout
        self.killed = False
        self.waits = 0

    def wait(self, timeout):
        self.waits += 1
        if self.timeout and self.waits == 1:
            raise subprocess.TimeoutExpired(["owned-renderer"], timeout)
        self.returncode = 1 if self.timeout else 0
        return self.returncode

    def poll(self):
        return self.returncode

    def kill(self):
        self.killed = True
        self.returncode = 1


class Job:
    def __init__(self, assignment_error=False):
        self.assignment_error = assignment_error
        self.assigned = None
        self.terminated = False
        self.closed = False

    def assign(self, process):
        if self.assignment_error:
            raise OSError("restricted parent job")
        self.assigned = process

    def terminate(self):
        self.terminated = True

    def active(self):
        return 0

    def stamp(self):
        return 123, 1000

    def close(self):
        self.closed = True


class ProductionWatchdogTests(unittest.TestCase):
    @unittest.skipUnless(PROCESS.os.name == "nt" and ctypes.sizeof(ctypes.c_void_p) == 8, "Windows x64 ABI only")
    def test_windows_job_structures_match_the_x64_sdk_layout(self):
        self.assertEqual(ctypes.sizeof(PROCESS._BasicLimits), 64)
        self.assertEqual(ctypes.sizeof(PROCESS._ExtendedLimits), 144)
        self.assertEqual(ctypes.sizeof(PROCESS._Accounting), 48)

    def invoke(self, process, job):
        with mock.patch.object(PROCESS.os, "name", "nt"), mock.patch.object(PROCESS, "_Job", return_value=job), \
                mock.patch.object(PROCESS.subprocess, "CREATE_NO_WINDOW", 0x08000000, create=True), \
                mock.patch.object(PROCESS.subprocess, "Popen", return_value=process) as popen:
            result = PROCESS.run_owned(["owned-renderer", "--role", "renderer"], 180, io.BytesIO(), io.BytesIO())
        self.assertEqual(popen.call_args.args[0], ["owned-renderer", "--role", "renderer"])
        self.assertNotIn("shell", popen.call_args.kwargs)
        return result

    def test_normal_exit_reaps_only_the_new_job_and_preserves_clock_metadata(self):
        process, job = Process(), Job()
        result = self.invoke(process, job)
        self.assertIs(job.assigned, process)
        self.assertTrue(job.closed)
        self.assertFalse(job.terminated)
        self.assertFalse(result["forced_cleanup"])
        self.assertEqual(result["exit_observed_qpc"], 123)
        self.assertFalse(result["job_assignment_atomic"])

    def test_timeout_terminates_only_the_owned_job_and_stays_failed(self):
        process, job = Process(timeout=True), Job()
        result = self.invoke(process, job)
        self.assertTrue(job.terminated)
        self.assertTrue(job.closed)
        self.assertEqual(process.waits, 2)
        self.assertTrue(result["timed_out"])
        self.assertTrue(result["forced_cleanup"])

    def test_assignment_failure_cleans_the_retained_child_handle_without_broadening_scope(self):
        process, job = Process(), Job(assignment_error=True)
        with self.assertRaises(OSError):
            self.invoke(process, job)
        self.assertTrue(process.killed)
        self.assertTrue(job.closed)
        self.assertIsNone(job.assigned)

    def test_trace_start_occurs_after_job_assignment_and_callback_failure_cleans_owned_renderer(self):
        process, job = Process(), Job()
        def started(pid):
            self.assertEqual(pid, 123)
            self.assertIs(job.assigned, process)
            raise RuntimeError('trace startup failed')
        with mock.patch.object(PROCESS.os, 'name', 'nt'), mock.patch.object(PROCESS, '_Job', return_value=job), \
                mock.patch.object(PROCESS.subprocess, 'CREATE_NO_WINDOW', 0, create=True), \
                mock.patch.object(PROCESS.subprocess, 'Popen', return_value=process):
            with self.assertRaisesRegex(RuntimeError, 'trace startup failed'):
                PROCESS.run_owned(['owned-renderer'], 180, io.BytesIO(), io.BytesIO(), started)
        self.assertTrue(process.killed)
        self.assertTrue(job.closed)


if __name__ == "__main__":
    unittest.main()
