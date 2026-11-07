import { ReadQueue } from './bounded-read-queue.ts';

/** Transport/document lifetime needed by a live editor, without a concrete transport. */
export interface EditorReadHost {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly projectGeneration?: number | null;
	readonly loading: { readonly visible: boolean };
	readonly currentHistoryStateId?: unknown;
	readonly hasActiveEditSession?: boolean;
	readonly editSessionEpoch?: number;
}

/** One live observation plus one latest replacement. Working history IDs advance during a
 * group; the group epoch fences those reads until closure restores exact history fencing. */
export class EditorReadQueue extends ReadQueue {
	observe<T>(
		host: EditorReadHost,
		query: string,
		current: () => boolean,
		read: () => Promise<T>,
		accept: (value: T) => void,
		reject: (error: unknown) => void,
		finish: () => void
	) {
		const generation = host.snapshotGeneration,
			project = host.projectGeneration,
			editing = host.hasActiveEditSession === true,
			revision = editing ? host.editSessionEpoch : host.currentHistoryStateId;
		const valid = () =>
			current() &&
			project !== null &&
			host.status === 'connected' &&
			!host.loading.visible &&
			host.snapshotGeneration === generation &&
			host.projectGeneration === project &&
			(host.hasActiveEditSession === true) === editing &&
			(editing ? host.editSessionEpoch === revision : host.currentHistoryStateId === revision);
		this.refresh(
			[host, generation, project, query, editing, revision],
			valid,
			read,
			(value) => {
				if (valid()) accept(value);
			},
			(error) => {
				if (valid()) reject(error);
			},
			() => {
				if (valid()) finish();
			}
		);
	}
}

/** Authored mutations remain blocked during another group; live presentation can still update. */
export const editorHistoryMatches = (host: EditorReadHost, history: unknown) =>
	host.hasActiveEditSession === true || host.currentHistoryStateId === history;
