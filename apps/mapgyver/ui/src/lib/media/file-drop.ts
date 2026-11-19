import { appState, type FileDropDestination, type WorkbenchSession } from 'golden_ui';

export function mediaFileDrop(
	feedback: Omit<FileDropDestination, 'drop'>,
	submit: (path: string, session: WorkbenchSession) => Promise<void>
): FileDropDestination | null {
	const session = appState.session;
	if (session?.status !== 'connected' || session.loading.visible || session.hasActiveEditSession)
		return null;
	const generation = session.snapshotGeneration;
	return {
		...feedback,
		async drop(paths) {
			if (paths.length !== 1) throw new Error('Drop one file at a time.');
			if (
				appState.session !== session ||
				session.snapshotGeneration !== generation ||
				session.status !== 'connected' ||
				session.loading.visible ||
				session.hasActiveEditSession
			)
				return;
			await submit(paths[0], session);
		}
	};
}
