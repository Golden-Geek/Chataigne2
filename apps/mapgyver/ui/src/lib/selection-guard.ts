/** Fence asynchronous creation selection against user input, project replacement and disconnect. */
export interface SelectionSession {
	readonly snapshotGeneration: number;
	readonly selectionRevision: number;
	readonly status: string;
	readonly loading: { readonly visible: boolean };
}

export function captureSelectionGuard<S extends SelectionSession>(session: S | null) {
	const generation = session?.snapshotGeneration;
	const revision = session?.selectionRevision;
	return (current: S | null): boolean =>
		current !== null &&
		current === session &&
		current.snapshotGeneration === generation &&
		current.selectionRevision === revision &&
		current.status === 'connected' &&
		!current.loading.visible;
}
