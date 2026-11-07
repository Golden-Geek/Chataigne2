import type { NodeId, ParamValue } from '../../types';
import type { GraphState } from '../graph.svelte';
import type { UiEditOperations } from './edit-operations';
import type { AppliedIntentObserver } from './intent-sender';

interface Host {
	readonly status: string;
	readonly snapshotGeneration: number;
	readonly loading: { readonly visible: boolean };
	readonly graph: { readonly state: GraphState };
	readonly editOperations: UiEditOperations;
}

/** Publish ordinary Parameter writes to the existing session draft owner. The original
 * dispatch still owns mutation ordering, validation, history, rejection and coalescing. */
export async function previewParameterWrite(
	host: Host | null,
	node: NodeId,
	value: ParamValue | undefined,
	dispatch: (applied?: AppliedIntentObserver) => Promise<boolean>,
	applied?: AppliedIntentObserver
): Promise<boolean> {
	const uuid = host?.graph.state.nodesById.get(node)?.uuid;
	if (!host || !uuid || !value) return dispatch(applied);
	const generation = host.snapshotGeneration;
	let completion = Promise.resolve(false),
		accepted = true;
	const operation = host.editOperations.open({
		current: () =>
			host.status === 'connected' &&
			!host.loading.visible &&
			host.snapshotGeneration === generation &&
			host.graph.state.nodesById.get(node)?.uuid === uuid,
		build: () => ({
			get accepted() {
				return accepted;
			},
			async finish() {
				accepted = await completion;
			},
			async cancel() {}
		})
	});
	operation?.publish([{ node, value }]);
	try {
		completion = dispatch((acks) => {
			operation?.applied(acks);
			applied?.(acks);
		});
		await operation?.confirm();
		return await completion;
	} catch (error) {
		await operation?.cancel();
		throw error;
	}
}

/** Presentation of a component intent; the actual request still edits only one backend axis. */
export function parameterComponentPreview(
	host: Host | null,
	node: NodeId,
	component: number,
	value: number
): ParamValue | undefined {
	const uuid = host?.graph.state.nodesById.get(node)?.uuid;
	const current = uuid
		? (host?.editOperations.get(node, uuid) ?? host?.graph.state.paramsById.get(node)?.value)
		: undefined;
	if (
		!current ||
		(current.kind !== 'vec2' && current.kind !== 'vec3') ||
		!Number.isInteger(component) ||
		component < 0 ||
		component >= current.value.length
	)
		return undefined;
	if (current.kind === 'vec2') {
		const next: [number, number] = [...current.value];
		next[component] = value;
		return { kind: 'vec2', value: next };
	}
	const next: [number, number, number] = [...current.value];
	next[component] = value;
	return { kind: 'vec3', value: next };
}
