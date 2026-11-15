/** Scalar lifetime identity, independent of reactive facade or loading-metadata object identity. */
export interface EditorScope {
	host: object | null;
	project: number | null;
	snapshot: number;
	status: string;
	loading: boolean;
	owner: string | null;
}

export function createEditorScopeObserver(replace: () => void) {
	let previous: EditorScope | null = null;
	return (next: EditorScope): void => {
		const old = previous;
		previous = { ...next };
		if (
			old &&
			(old.host !== next.host ||
				old.project !== next.project ||
				old.snapshot !== next.snapshot ||
				old.status !== next.status ||
				old.loading !== next.loading ||
				old.owner !== next.owner)
		)
			replace();
	};
}
