import { createUiEditSession, sendCreateUserItemIntent } from '../../../store/ui-intents';
import type { UiCreatableUserItem } from '../../../types';
import { readCurveKeys } from './model';
import type { CurveOperation } from './operations';
import type { CurveCreatePlan, ParsedCurveKey } from './types';

interface Context {
	ready(plan: CurveCreatePlan, key: ParsedCurveKey, keys: Map<number, ParsedCurveKey>): boolean;
	initialize(
		scope: CurveOperation,
		plan: CurveCreatePlan,
		key: ParsedCurveKey,
		keys: Map<number, ParsedCurveKey>
	): Promise<void>;
	preview(plan: CurveCreatePlan, key: ParsedCurveKey): () => void;
	select(scope: CurveOperation, key: ParsedCurveKey): void;
}
interface Pending {
	scope: CurveOperation;
	plan: CurveCreatePlan;
	created: { node_id: number; uuid: string };
	processing: boolean;
	select: boolean;
	finish(key: ParsedCurveKey | null): void;
}

/** Match the exact creation receipt; wait for projection events instead of polling for any new key. */
export function createCurveCreation(context: Context) {
	let busy = $state(false);
	let pending = $state.raw<Pending | null>(null);
	$effect(() => {
		const value = pending;
		if (!value || value.processing) return;
		if (!value.scope.documentCurrent()) {
			value.finish(null);
			return;
		}
		const nodes = value.scope.host.graph.state.nodesById,
			owner = nodes.get(value.scope.owner.node_id);
		const created = nodes.get(value.created.node_id);
		if (!owner || created?.uuid !== value.created.uuid || !owner.children.includes(created.node_id))
			return;
		const keys = new Map(
			readCurveKeys(nodes, owner, new Map(), new Map()).map((key) => [key.node_id, key])
		);
		const key = keys.get(created.node_id);
		if (!key || !context.ready(value.plan, key, keys)) return;
		value.processing = true;
		void (async () => {
			let clear: (() => void) | undefined;
			try {
				clear = context.preview(value.plan, key);
				await context.initialize(value.scope, value.plan, key, keys);
				if (value.select && value.scope.selectionCurrent()) context.select(value.scope, key);
				value.finish(value.scope.documentCurrent() ? key : null);
			} catch (error) {
				value.finish(null);
				console.error('Curve key initialization failed', error);
			} finally {
				clear?.();
			}
		})();
	});
	return {
		get busy() {
			return busy;
		},
		select: context.select,
		async create(
			scope: CurveOperation,
			item: UiCreatableUserItem,
			plan: CurveCreatePlan,
			select = true
		): Promise<ParsedCurveKey | null> {
			if (busy || !scope.current()) return null;
			busy = true;
			const group = createUiEditSession('Add Curve Key', 'curve-key-add', scope.host);
			let abort: (() => void) | null = null;
			try {
				await group.begin();
				if (!scope.current() || !group.active) return null;
				const result = await sendCreateUserItemIntent(scope.owner.node_id, item);
				if (
					!result.success ||
					!scope.documentCurrent() ||
					result.createdNodeId === null ||
					result.createdNodeUuid === null
				)
					return null;
				return await new Promise<ParsedCurveKey | null>((resolve) => {
					const finish = (key: ParsedCurveKey | null) => {
						if (pending?.scope === scope) pending = null;
						resolve(key);
					};
					abort = () => {
						if (!pending?.processing) finish(null);
					};
					scope.signal.addEventListener('abort', abort, { once: true });
					pending = {
						scope,
						plan,
						select,
						created: { node_id: result.createdNodeId!, uuid: result.createdNodeUuid! },
						processing: false,
						finish
					};
				});
			} finally {
				if (abort) scope.signal.removeEventListener('abort', abort);
				try {
					await group.end();
				} finally {
					busy = false;
				}
			}
		}
	};
}
