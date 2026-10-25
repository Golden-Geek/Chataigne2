import type { UiEditIntent, EventTime, ParamValue } from '../../../types';
import type { createFixtureSession } from './session';
import {
	getDirectParamNode,
	getDashboardLayoutKind
} from '../../../components/panels/dashboard/dashboard-model';

/** Controlled receipts/publications from the real pipeline, separate from backend policy tests. */
export function createDashboardEditIO(
	fixture: ReturnType<typeof createFixtureSession>,
	time: () => EventTime
) {
	let holdBegin = false,
		holdWrites = false,
		rejectWrites = false,
		holdEvents = false;
	const records: UiEditIntent[] = [],
		pending: (() => void)[] = [],
		events: (() => void)[] = [];
	return {
		configure(
			options: {
				holdBegin?: boolean;
				holdWrites?: boolean;
				rejectWrites?: boolean;
				holdEvents?: boolean;
			} = {}
		) {
			if (pending.length || events.length)
				throw new Error('Resolve edit receipts/events before resetting the fixture');
			holdBegin = options.holdBegin ?? false;
			holdWrites = options.holdWrites ?? false;
			rejectWrites = options.rejectWrites ?? false;
			holdEvents = options.holdEvents ?? false;
			records.length = 0;
		},
		handle(intent: UiEditIntent) {
			if (
				![
					'beginEdit',
					'endEdit',
					'setParam',
					'resizeDashboardWidget',
					'moveDashboardWidget'
				].includes(intent.kind)
			)
				return null;
			records.push(structuredClone(intent));
			const write = intent.kind !== 'beginEdit' && intent.kind !== 'endEdit';
			const rejected = write && rejectWrites;
			const times: EventTime[] = [];
			const enqueue = (apply: (at: EventTime) => void) => {
				const at = time();
				times.push(at);
				if (holdEvents) events.push(() => apply(at));
				else apply(at);
			};
			const value = (id: number, next: ParamValue) =>
				enqueue((at) => {
					const node = fixture.session.graph.state.nodesById.get(id);
					if (node?.data.kind !== 'parameter') throw new Error('Edit reached a retired parameter');
					fixture.session.graph.applyEvent({
						time: at,
						kind: {
							kind: 'paramChanged',
							param: id,
							old_value: node.data.param.value,
							new_value: next
						}
					});
				});
			if (!rejected && intent.kind === 'setParam') value(intent.node, intent.value);
			if (!rejected && intent.kind === 'resizeDashboardWidget') {
				for (const axis of ['width', 'height'] as const) {
					const next = intent[axis];
					if (!next) continue;
					const graph = fixture.session.graph.state;
					const field = getDirectParamNode(graph, graph.nodesById.get(intent.widget)!, axis);
					if (!field) throw new Error('Missing fixture resize field');
					enqueue((at) =>
						fixture.session.graph.applyEvent({
							time: at,
							kind: { kind: 'metaChanged', node: field.node_id, patch: { enabled: true } }
						})
					);
					value(field.node_id, { kind: 'css_value', ...next });
				}
			}
			if (!rejected && intent.kind === 'moveDashboardWidget') {
				enqueue((at) => {
					const graph = fixture.session.graph.state;
					const parent = graph.nodesById.get(intent.parent)!;
					const oldParent = graph.parentById.get(intent.widget)!;
					const others = parent.children.filter((id) => id !== intent.widget);
					const items = others.filter((id) => graph.nodesById.get(id)?.user_role === 'itemRoot');
					const index = Math.min(items.length, intent.target_index ?? items.length);
					const before = items[index];
					const offset = before === undefined ? others.length : others.indexOf(before);
					const children = [...others.slice(0, offset), intent.widget, ...others.slice(offset)];
					fixture.session.graph.applyEvent({
						time: at,
						kind:
							oldParent === intent.parent
								? {
										kind: 'childReordered',
										parent: intent.parent,
										child: intent.widget,
										parent_children: children
									}
								: {
										kind: 'childMoved',
										child: intent.widget,
										old_parent: oldParent,
										new_parent: intent.parent,
										old_parent_children: graph.nodesById
											.get(oldParent)!
											.children.filter((id) => id !== intent.widget),
										new_parent_children: children
									}
					});
				});
				// Controlled authoritative publications; backend policy is qualified by the Rust suite.
				const placement = intent.placement;
				if (placement) {
					const graph = fixture.session.graph.state,
						owner = graph.nodesById.get(intent.widget)!;
					const layout = getDashboardLayoutKind(graph, graph.nodesById.get(intent.parent)!);
					const edits: [string, ParamValue, boolean | undefined][] = [];
					if (layout === 'free' || layout === 'horizontal')
						edits.push([
							'width',
							{ kind: 'css_value', ...placement.width },
							layout === 'free' || placement.size_enabled?.width === true
						]);
					if (layout === 'free' || layout === 'vertical')
						edits.push([
							'height',
							{ kind: 'css_value', ...placement.height },
							layout === 'free' || placement.size_enabled?.height === true
						]);
					if (layout === 'free')
						edits.push(
							['position', { kind: 'vec2', value: placement.position }, undefined],
							['anchor', { kind: 'enum', value: placement.anchor }, undefined]
						);
					for (const [key, next, enabled] of edits) {
						const field = getDirectParamNode(graph, owner, key);
						if (!field) continue;
						if (enabled !== undefined)
							enqueue((at) =>
								fixture.session.graph.applyEvent({
									time: at,
									kind: { kind: 'metaChanged', node: field.node_id, patch: { enabled } }
								})
							);
						value(field.node_id, next);
					}
				}
			}
			const reply = () => ({
				...fixture.editAcknowledgement(
					intent.kind === 'beginEdit' ? true : intent.kind === 'endEdit' ? false : undefined
				),
				...(times.length ? { earliest_event_time: times[0], latest_event_time: times.at(-1) } : {}),
				...(rejected
					? {
							success: false,
							status: 'rejected' as const,
							error_message: 'Fixture parameter rejection'
						}
					: {})
			});
			if ((intent.kind === 'beginEdit' && holdBegin) || (write && holdWrites))
				return new Promise<ReturnType<typeof reply>>((resolve) => {
					pending.push(() => resolve(reply()));
				});
			return Promise.resolve(reply());
		},
		release() {
			for (const resolve of pending.splice(0)) resolve();
		},
		releaseEvents() {
			for (const apply of events.splice(0)) apply();
		},
		read() {
			return {
				records: structuredClone(records),
				pending: pending.length,
				events: events.length,
				operations: fixture.session.editOperations.size,
				active: fixture.session.hasActiveEditSession
			};
		}
	};
}
