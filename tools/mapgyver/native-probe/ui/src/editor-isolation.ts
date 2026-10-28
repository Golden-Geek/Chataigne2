import { invoke } from './native';
import type { BusyCommand, EditorActivity } from './generated';

/** Finite, host-requested interruption of this owned qualification WebView only. */
export function observeEditorIsolation(session: () => number | undefined, resumed: () => void) {
  const handler = (event: Event) => {
    const command = (event as CustomEvent<BusyCommand>).detail;
    if (command.session !== session() || command.duration_ms !== 65_000) return;
    const started: EditorActivity = { kind: 'busy_started', session: command.session };
    void invoke('report_editor_activity', { event: started }).then(async () => {
      const start = performance.now();
      // Deliberate bounded JS-thread blockage: this is the native isolation test stimulus.
      while (performance.now() - start < command.duration_ms) { /* no IPC or rendering */ }
      const finished: EditorActivity = {
        kind: 'busy_finished', session: command.session, elapsed_ms: performance.now() - start,
      };
      await invoke('report_editor_activity', { event: finished });
      resumed();
    }).catch((cause) => console.error('Editor isolation stimulus failed', cause));
  };
  window.addEventListener('probe-editor-busy', handler);
  return () => window.removeEventListener('probe-editor-busy', handler);
}
