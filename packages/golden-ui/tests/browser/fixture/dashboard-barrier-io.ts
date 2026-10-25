import {
	attemptSaveProjectFile,
	projectFileState,
	syncProjectFilePathFromSnapshot
} from '../../../store/project-files.svelte';
import type { createFixtureSession } from './session';

export function createDashboardBarrierIO(fixture: ReturnType<typeof createFixtureSession>) {
	const requests: Promise<unknown>[] = [],
		calls: { kind: string; active: boolean; history: number }[] = [];
	const held: (() => void)[] = [];
	let holdSave = false,
		rejectSave = false;
	const entered = (kind: string) =>
		calls.push({
			kind,
			active: fixture.session.hasActiveEditSession,
			history: fixture.session.currentHistoryStateId
		});
	fixture.handleProjectSave(async () => {
		entered('save');
		if (holdSave) await new Promise<void>((resolve) => held.push(resolve));
		if (rejectSave) throw new Error('Fixture save rejection');
	});
	return {
		configure(options: { holdSave?: boolean; rejectSave?: boolean } = {}) {
			if (held.length) throw new Error('Release the pending save before resetting');
			calls.length = 0;
			holdSave = options.holdSave ?? false;
			rejectSave = options.rejectSave ?? false;
			syncProjectFilePathFromSnapshot('fixture.fixture');
			projectFileState.savedHistoryStateId = 0;
		},
		request(
			kind: 'save' | 'undo' | 'redo' | 'project' | 'reparent',
			reparent: () => Promise<void>
		) {
			const id = requests.length;
			const session = fixture.session;
			requests.push(
				kind === 'save'
					? attemptSaveProjectFile()
					: kind === 'undo'
						? session.undo()
						: kind === 'redo'
							? session.redo()
							: kind === 'reparent'
								? session.runEditBarrier(async () => {
										entered(kind);
										await reparent();
									})
								: session.runProjectGraphTransition(async (control) => {
										entered(kind);
										control.markRuntimeGraphReplaced();
										if (!(await session.refreshSnapshotAfterCurrentSynchronization()))
											throw new Error('Project fixture snapshot failed');
									})
			);
			void requests[id].catch(() => {});
			return id;
		},
		wait: (id: number) => requests[id],
		release() {
			for (const resolve of held.splice(0)) resolve();
		},
		read: () => ({
			calls: structuredClone(calls),
			held: held.length,
			savedHistoryId: projectFileState.savedHistoryStateId,
			history: fixture.session.currentHistoryStateId,
			busy: projectFileState.busy
		})
	};
}
