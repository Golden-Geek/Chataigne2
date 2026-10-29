"""Optional owned PID-filtered trace; reuses the pinned collector and public query CLI."""

import base64
import codecs
import hashlib
from pathlib import Path
import subprocess
import time
import uuid

ROOT = Path(__file__).resolve().parents[2]
PRESENTMON = ROOT / 'target/qualification-tools/presentmon-2.6.0/PresentMon-2.6.0-x64.exe'
PRESENTMON_SHA256 = 'b2a706bc6ad475749e3b7e3409263aa1e6906d45bdcf993f6dbc0f660188f1af'
QUERY = ROOT / 'target/release/mapgyver_native_probe.exe'


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def decode_stream(data):
    if data.startswith((codecs.BOM_UTF16_LE, codecs.BOM_UTF16_BE)):
        return data.decode('utf-16'), 'utf-16-bom'
    return data.decode('utf-8-sig'), 'utf-8'


class Capture:
    def __init__(self, presentmon, query, prefix, seconds):
        self.presentmon, self.query, self.prefix = presentmon, query, prefix
        self.seconds = seconds + 40
        # The retained M01 probe exposes this read-only public command/owned namespace.
        # No legacy renderer, host or private source module is imported.
        self.session = 'MapGyver-M01-Present-' + uuid.uuid4().hex
        self.process = None
        self.streams = []
        self.report = {'requested': True, 'session': self.session, 'forced_cleanup': False}
        if not presentmon.is_file() or not query.is_file():
            raise ValueError('PresentMon capture requires the pinned collector and built trace-query executable')
        self.identities = {'presentmon_sha256': digest(presentmon), 'query_executable_sha256': digest(query)}
        if presentmon.resolve() == PRESENTMON.resolve() and self.identities['presentmon_sha256'] != PRESENTMON_SHA256:
            raise ValueError('pinned PresentMon checksum differs')
        help_result = subprocess.run([str(presentmon), '--help'], capture_output=True, timeout=5)
        help_text = (help_result.stdout + help_result.stderr).decode('utf-8-sig')
        if not all(option in help_text for option in ('--v1_metrics', '--qpc_time', '--session_name', '--process_id',
                                                     '--terminate_after_timed', '--output_stdout', '--no_track_input')):
            raise ValueError('collector lacks the required PresentMon contract')
        self.report['tools'] = self.identities

    def current_identities(self):
        return {'presentmon_sha256': digest(self.presentmon), 'query_executable_sha256': digest(self.query)}

    def start(self, pid):
        self.report['target_pid'] = pid
        command = [str(self.presentmon), '--process_id', str(pid), '--session_name', self.session,
                   '--output_stdout', '--v1_metrics', '--qpc_time', '--no_console_stats', '--no_track_input',
                   '--timed', str(self.seconds), '--terminate_after_timed']
        self.report['command'] = command
        try:
            for suffix in ('stdout.csv', 'stderr.log'):
                self.streams.append(Path(str(self.prefix) + '-' + suffix).open('xb'))
            self.process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=self.streams[0],
                                            stderr=self.streams[1], creationflags=subprocess.CREATE_NO_WINDOW)
            self.until = time.monotonic() + self.seconds + 15
        except (OSError, subprocess.SubprocessError) as error:
            self.report['error'] = str(error)

    def _stop_owned_session(self):
        command = [str(self.presentmon), '--session_name', self.session, '--terminate_existing_session']
        try:
            result = subprocess.run(command, capture_output=True, timeout=5)
            self.report['session_stop'] = {'command': command, 'exit_code': result.returncode,
                                          'stdout': result.stdout.decode('utf-8-sig'),
                                          'stderr': result.stderr.decode('utf-8-sig')}
        except (OSError, subprocess.SubprocessError, UnicodeError) as error:
            self.report['session_stop'] = {'command': command, 'error': str(error)}

    def finish(self, native_failed=False):
        try:
            if self.process is not None:
                command = [str(self.query), 'trace-status', self.session]
                try:
                    result = subprocess.run(command, capture_output=True, timeout=5)
                    self.report['live_query'] = {'command': command, 'exit_code': result.returncode,
                                                'stdout': result.stdout.decode('utf-8-sig'),
                                                'stderr': result.stderr.decode('utf-8-sig')}
                except (OSError, subprocess.SubprocessError, UnicodeError) as error:
                    self.report['live_query'] = {'command': command, 'error': str(error)}
                if native_failed:
                    self._stop_owned_session()
                try:
                    self.process.wait(timeout=10 if native_failed else max(1, self.until-time.monotonic()))
                except subprocess.TimeoutExpired:
                    self.report['timed_out'] = True
                    self._stop_owned_session()
                    if self.process.poll() is None:
                        self.report['forced_cleanup'] = True
                        self.process.kill()
                    self.process.wait(timeout=10)
                self.report['exit_code'] = self.process.returncode
        finally:
            for stream in self.streams:
                stream.close()
        for field, suffix in (('stdout', 'stdout.csv'), ('stderr', 'stderr.log')):
            path = Path(str(self.prefix) + '-' + suffix)
            raw = path.read_bytes() if path.exists() else b''
            self.report[field], encoding = decode_stream(raw)
            self.report[field + '_raw'] = {'encoding': encoding, 'bytes': len(raw),
                                          'sha256': hashlib.sha256(raw).hexdigest(),
                                          'base64': base64.b64encode(raw).decode('ascii')}
        self.report['tools_unchanged'] = self.identities == self.current_identities()
        return self.report
