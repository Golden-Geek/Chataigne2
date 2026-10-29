import importlib.util
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
SPEC = importlib.util.spec_from_file_location('production_pm_capture', ROOT/'tools/mapgyver/production_presentmon_capture.py')
CAPTURE = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(CAPTURE)
HELP = b'--v1_metrics --qpc_time --session_name --process_id --terminate_after_timed --output_stdout --no_track_input'


class Process:
    returncode = None

    def __init__(self, timeout):
        self.timeout, self.killed = timeout, False

    def wait(self, timeout):
        if self.timeout:
            self.timeout = False
            raise subprocess.TimeoutExpired('owned collector', timeout)
        self.returncode = 1 if self.killed else 0

    def poll(self):
        return self.returncode

    def kill(self):
        self.killed = True


class PresentMonCaptureTests(unittest.TestCase):
    def test_redirected_utf16_and_utf8_streams_are_decoded_strictly_without_numeric_conversion(self):
        text = 'QPCTime,SwapChainAddress\r\n3037592146544,0x1234\r\n'
        for encoding in ('utf-8-sig', 'utf-16', 'utf-16-be'):
            raw = text.encode(encoding)
            if encoding == 'utf-16-be':
                raw = b'\xfe\xff' + raw
            self.assertEqual(CAPTURE.decode_stream(raw)[0], text)
        with self.assertRaises(UnicodeDecodeError):
            CAPTURE.decode_stream(b'\xff\xfe\x00')

    def invoke(self, timeout):
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            pm, query = root/'collector.exe', root/'query.exe'
            pm.write_bytes(b'collector fixture')
            query.write_bytes(b'query fixture')
            commands, process = [], Process(timeout)
            def run(command, **kwargs):
                commands.append(command)
                return mock.Mock(returncode=0, stdout=HELP if command[-1] == '--help' else b'{}', stderr=b'')
            with mock.patch.object(CAPTURE.subprocess, 'run', side_effect=run), \
                    mock.patch.object(CAPTURE.subprocess, 'Popen', return_value=process) as popen, \
                    mock.patch.object(CAPTURE.subprocess, 'CREATE_NO_WINDOW', 0, create=True):
                capture = CAPTURE.Capture(pm, query, root/'raw', 10)
                capture.start(123)
                report = capture.finish()
            command = popen.call_args.args[0]
            self.assertEqual(command[command.index('--process_id')+1], '123')
            self.assertNotIn('--restart_as_admin', command)
            self.assertNotIn('--exclude_dropped', command)
            self.assertRegex(report['session'], r'^MapGyver-M01-Present-[0-9a-f]{32}$')
            self.assertEqual(commands[1], [str(query), 'trace-status', report['session']])
            self.assertTrue(report['tools_unchanged'])
            return report, commands, process

    def test_normal_capture_queries_only_its_unique_session_and_does_not_stop_other_sessions(self):
        report, commands, process = self.invoke(False)
        self.assertEqual(len(commands), 2)
        self.assertFalse(process.killed)
        self.assertFalse(report['forced_cleanup'])

    def test_timeout_stops_only_its_unique_session_and_retained_collector_handle(self):
        report, commands, process = self.invoke(True)
        self.assertEqual(commands[2][1:], ['--session_name', report['session'], '--terminate_existing_session'])
        self.assertTrue(process.killed)
        self.assertTrue(report['timed_out'])
        self.assertTrue(report['forced_cleanup'])
