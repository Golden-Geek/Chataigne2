/** Session capabilities used by app-owned readers and intent controllers. */
export interface Connection {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly currentHistoryStateId?: unknown;
	readonly loading: { readonly visible: boolean };
	readonly hasActiveEditSession?: boolean;
}
