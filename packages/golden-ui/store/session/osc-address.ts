import type { NodeId } from '../../types';
import type { WorkbenchSession } from '../workbench/session-types';

export const requestOscAddress = async (
	session: WorkbenchSession,
	nodeId: NodeId,
	signal?: AbortSignal
): Promise<string | null> => {
	const { client, projectGeneration, snapshotGeneration } = session;
	const uuid = session.graph.state.nodesById.get(nodeId)?.uuid;
	if (!uuid || !client.oscAddress || projectGeneration === null || session.loading.visible)
		return null;
	const result = await client.oscAddress(
		{ node: nodeId, project_generation: projectGeneration },
		signal
	);
	return !signal?.aborted &&
		session.status === 'connected' &&
		!session.loading.visible &&
		session.snapshotGeneration === snapshotGeneration &&
		session.projectGeneration === projectGeneration &&
		session.graph.state.nodesById.get(nodeId)?.uuid === uuid
		? result.address
		: null;
};
