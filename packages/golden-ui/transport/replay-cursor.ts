import type { UiReplayCursor, UiSnapshot } from '../types';

export function projectGeneration(value: unknown): number {
	if (typeof value !== 'number' || !Number.isSafeInteger(value) || value < 1)
		throw new Error('Transport project generation must be a positive safe integer.');
	return value;
}

/** Snapshot nodes, project identity and event boundary always travel together. */
export function snapshotCursor(
	snapshot: Pick<UiSnapshot, 'at' | 'project_generation'>
): UiReplayCursor {
	return { project_generation: projectGeneration(snapshot.project_generation), at: snapshot.at };
}
