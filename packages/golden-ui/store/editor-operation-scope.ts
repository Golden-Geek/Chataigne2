import type { EditorReadHost } from './editor-read-queue';

export interface EditorOperationHost extends EditorReadHost {
	readonly projectGeneration: number | null;
	readonly selectionRevision: number;
}
export interface EditorOperation<H extends EditorOperationHost> {
	readonly host: H;
	readonly signal: AbortSignal;
	current(): boolean;
	documentCurrent(): boolean;
	selectionCurrent(): boolean;
	release(): void;
}

/** Pending commands retain their document; admitted work may settle after the view moves on. */
export function createEditorOperationScopes<H extends EditorOperationHost>(
	currentHost: () => H | null
) {
	let disposed = false;
	const observers = new Set<() => void>();
	return {
		capture(currentOwner: () => boolean, availableOwner: () => boolean): EditorOperation<H> | null {
			const host = currentHost();
			if (
				disposed ||
				!host ||
				host.status !== 'connected' ||
				host.loading.visible ||
				host.projectGeneration === null
			)
				return null;
			const generation = host.snapshotGeneration,
				project = host.projectGeneration,
				selection = host.selectionRevision;
			const controller = new AbortController();
			let viewLost = false;
			const documentCurrent = () =>
				!disposed &&
				!controller.signal.aborted &&
				currentHost() === host &&
				host.status === 'connected' &&
				!host.loading.visible &&
				host.snapshotGeneration === generation &&
				host.projectGeneration === project &&
				availableOwner();
			const current = () => {
				if (!currentOwner()) viewLost = true;
				return !viewLost && documentCurrent();
			};
			const observe = () => {
				current();
				if (!documentCurrent()) controller.abort();
			};
			observers.add(observe);
			return {
				host,
				signal: controller.signal,
				current,
				documentCurrent,
				selectionCurrent: () => current() && host.selectionRevision === selection,
				release() {
					observers.delete(observe);
					controller.abort();
				}
			};
		},
		observe() {
			for (const observe of observers) observe();
		},
		dispose() {
			disposed = true;
			for (const observe of observers) observe();
			observers.clear();
		}
	};
}
