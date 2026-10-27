import type { EventTime, UiAck, UiNodeDto, WorkbenchSession } from 'golden_ui';
import { compareEventTime } from 'golden_ui/store/session/event-time';
import type { SpatializerEdit } from './generated/SpatializerEdit';
import type { SpatializerEditResult } from './generated/SpatializerEditResult';
import type { SpatializerIdentity } from './generated/SpatializerIdentity';
import type { SpatializerSpace } from './generated/SpatializerSpace';
import { SPATIALIZER_NAMESPACE, SPATIALIZER_EDIT_OPERATION } from './generated/constants';

export type SpatializerSession = Pick<
	WorkbenchSession,
	'graph' | 'snapshotGeneration' | 'editOperations' | 'sendIntent' | 'invokeAppOperation'
>;
interface Source {
	session: SpatializerSession | null;
	module: UiNodeDto | null;
	space: SpatializerSpace;
}
const record = (value: unknown): value is Record<string, unknown> =>
	typeof value === 'object' && value !== null && !Array.isArray(value);
const identity = (value: unknown): value is SpatializerIdentity =>
	record(value) &&
	Number.isSafeInteger(value.node) &&
	typeof value.uuid === 'string' &&
	value.uuid.length > 0;
const same = (a: SpatializerIdentity, b: SpatializerIdentity) =>
	a.node === b.node && a.uuid === b.uuid;
const targets = (edit: SpatializerEdit) =>
	edit.kind === 'move'
		? edit.moves.map((move) => move.endpoint)
		: edit.kind === 'createTarget'
			? []
			: [edit.endpoint];

/** Captures owner/input before bounded session admission. A gesture retains one latest unsent sample. */
export function createSpatializerAuthoring(source: () => Source) {
	let disposed = false;
	const cancellations = new Set<() => Promise<void>>();
	const capture = () => {
		const current = source();
		if (disposed || !current.session || !current.module)
			throw new Error('Spatializer editing requires a module and session.');
		return {
			session: current.session,
			generation: current.session.snapshotGeneration,
			module: { node: current.module.node_id, uuid: current.module.uuid },
			space: current.space
		};
	};
	type Owner = ReturnType<typeof capture>;
	const current = (owner: Owner) => {
		const next = source();
		return (
			!disposed &&
			next.session === owner.session &&
			next.module?.node_id === owner.module.node &&
			next.module.uuid === owner.module.uuid &&
			next.space === owner.space &&
			owner.session.snapshotGeneration === owner.generation
		);
	};
	const dispatch = async (
		owner: Owner,
		edit: SpatializerEdit,
		applied?: (acks: readonly UiAck[]) => void
	): Promise<SpatializerEditResult> => {
		if (!current(owner)) throw new Error('Spatializer owner changed before dispatch.');
		for (const target of targets(edit)) {
			const node = owner.session.graph.state.nodesById.get(target.node);
			if (node?.uuid !== target.uuid)
				throw new Error('Spatializer endpoint identity changed before dispatch.');
		}
		const result = await owner.session.invokeAppOperation(
			{
				kind: 'invokeAppOperation',
				namespace: SPATIALIZER_NAMESPACE,
				operation: SPATIALIZER_EDIT_OPERATION,
				payload: { module: owner.module, space: owner.space, edit }
			},
			applied
		);
		if (
			!record(result) ||
			!Array.isArray(result.endpoints) ||
			!result.endpoints.every(identity) ||
			(result.created !== null && !identity(result.created))
		)
			throw new Error('Spatializer edit returned an invalid committed result.');
		const expected = targets(edit);
		if (edit.kind === 'createTarget') {
			if (
				!identity(result.created) ||
				result.endpoints.length !== 1 ||
				!same(result.endpoints[0], result.created)
			)
				throw new Error('Spatializer creation did not return one committed identity.');
		} else if (
			result.created !== null ||
			result.endpoints.length !== expected.length ||
			result.endpoints.some((value, index) => !same(value, expected[index]))
		) {
			throw new Error('Spatializer edit returned different endpoint identities.');
		}
		return { endpoints: result.endpoints, created: result.created };
	};
	return {
		async run(
			edit: SpatializerEdit,
			published?: (boundary: EventTime | undefined) => void
		): Promise<SpatializerEditResult> {
			const owner = capture(),
				request = structuredClone(edit);
			let result: SpatializerEditResult | undefined;
			let boundary: EventTime | undefined;
			const operation = owner.session.editOperations.open({
				current: () => current(owner),
				build: (scope) => ({
					accepted: true,
					async finish() {
						await scope.ready;
						if (!scope.current) throw new Error('Spatializer request was retired.');
						result = await dispatch(owner, request, (acks) => {
							scope.applied(acks);
							for (const ack of acks) {
								const at = ack.latest_event_time ?? ack.earliest_event_time ?? undefined;
								if (at && (!boundary || compareEventTime(at, boundary) > 0)) boundary = at;
							}
						});
					},
					async cancel() {}
				})
			});
			if (!operation) throw new Error('Spatializer editing is busy or crossing a project barrier.');
			const cancel = () => operation.cancel();
			cancellations.add(cancel);
			try {
				await operation.confirm();
				if (!result) throw new Error('Spatializer request was cancelled.');
				published?.(boundary);
				return result;
			} finally {
				cancellations.delete(cancel);
			}
		},
		gesture(target: UiNodeDto, field: UiNodeDto, failed: (error: unknown) => void) {
			const owner = capture();
			const expected = { node: target.node_id, uuid: target.uuid },
				parameter = { node: field.node_id, uuid: field.uuid };
			let latest: SpatializerEdit | null = null,
				flight: Promise<void> | null = null;
			let stopping = false,
				begun = false,
				failure: unknown;
			let boundary: EventTime | undefined;
			let aborted = false;
			let retire = () => {};
			const clientEditId = `spatializer-${crypto.randomUUID()}`;
			const live = () => {
				const node = owner.session.graph.state.nodesById.get(parameter.node);
				return (
					current(owner) &&
					owner.session.graph.state.nodesById.get(expected.node)?.uuid === expected.uuid &&
					node?.uuid === parameter.uuid &&
					node.data.kind === 'parameter' &&
					!node.data.param.read_only
				);
			};
			const operation = owner.session.editOperations.open({
				current: live,
				build: (scope) => {
					const fail = (error: unknown) => {
						if (failure) return;
						failure = error instanceof Error ? error : new Error(String(error));
						stopping = true;
						latest = null;
						failed(failure);
						retire();
					};
					const applied = (acks: readonly UiAck[]) => {
						scope.applied(acks);
						for (const ack of acks) {
							const at = ack.latest_event_time ?? ack.earliest_event_time ?? undefined;
							if (at && (!boundary || compareEventTime(at, boundary) > 0)) boundary = at;
						}
					};
					const ready = scope.ready.then(async () => {
						if (stopping || !scope.current) return;
						await owner.session.sendIntent(
							{
								kind: 'beginEdit',
								client_edit_id: clientEditId,
								label: `Edit ${target.meta.label}`
							},
							applied
						);
						begun = true;
					});
					void ready.catch(fail);
					const close = async () => {
						// A projection refresh retires edits, not the engine history group. The
						// unique id lets the engine reject a stale close after project replacement.
						if (begun) {
							begun = false;
							await owner.session.sendIntent(
								{ kind: 'endEdit', client_edit_id: clientEditId },
								applied
							);
						}
					};
					const pump = () => {
						if (flight || stopping) return;
						flight = ready
							.then(async () => {
								while (latest && !stopping && scope.current) {
									const edit = latest;
									latest = null;
									await dispatch(owner, edit, applied);
								}
							})
							.catch(fail)
							.finally(() => {
								flight = null;
							});
					};
					let finalization: Promise<void> | null = null;
					const finalize = (cancelled: boolean) => {
						if (cancelled) {
							aborted = true;
							stopping = true;
							latest = null;
						}
						return (finalization ??= (async () => {
							try {
								await ready.catch((error) => {
									if (!cancelled) throw failure ?? error;
								});
								while (flight) await flight;
								if (!cancelled && latest && scope.current && !stopping) {
									const edit = latest;
									latest = null;
									await dispatch(owner, edit, applied);
								}
								if (!cancelled && failure) throw failure;
							} finally {
								stopping = true;
								latest = null;
								await close();
							}
						})());
					};
					return {
						accepted: true,
						offer(edit: SpatializerEdit) {
							if (stopping || !scope.current) throw new Error('Spatializer gesture was retired.');
							latest = structuredClone(edit);
							pump();
						},
						finish: () => finalize(false),
						cancel: () => finalize(true)
					};
				}
			});
			if (!operation) throw new Error('Spatializer editing is busy or crossing a project barrier.');
			const cancel = async () => {
				try {
					await operation.cancel();
				} finally {
					cancellations.delete(cancel);
				}
			};
			cancellations.add(cancel);
			retire = () => {
				void cancel().catch(() => undefined);
			};
			return {
				get current() {
					return operation.current;
				},
				get ownerCurrent() {
					return live();
				},
				offer: operation.work.offer,
				async finish() {
					try {
						await operation.confirm();
						if (failure) throw failure;
						if (aborted || !operation.confirmed)
							throw new Error('Spatializer gesture was retired.');
						return boundary;
					} finally {
						cancellations.delete(cancel);
					}
				},
				cancel
			};
		},
		dispose() {
			disposed = true;
			for (const cancel of cancellations) void cancel().catch(() => undefined);
		}
	};
}
export type SpatializerAuthoring = ReturnType<typeof createSpatializerAuthoring>;
