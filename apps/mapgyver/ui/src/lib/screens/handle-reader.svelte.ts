import { onDestroy } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { ReadQueue } from 'golden_ui/store/bounded-read-queue';
import { observeEditor } from 'golden_ui/store/editor-observation.svelte';
import { usePanelVisibility } from 'golden_ui/dockview/panel-visibility.svelte';
import { createEditorParameterProjection } from 'golden_ui/store/editor-parameter-projection.svelte';
import { invokeScreenEdit } from './operations';
import type { ScreenHandleQuery } from './generated/ScreenHandleQuery';
import type { ScreenHandlePage } from './generated/ScreenHandlePage';

/** One bounded, lifecycle-fenced metadata page; no other Screen preview is opened. */
export function createScreenHandles(
	query: () => ScreenHandleQuery | null,
	interacting: () => boolean
) {
	const visibility = usePanelVisibility();
	const session = $derived(appState.session);
	const key = $derived(JSON.stringify(query()));
	const queue = new ReadQueue();
	const projection = createEditorParameterProjection(() => session);
	let result = $state.raw<{
		session: WorkbenchSession;
		key: string;
		project: number;
		generation: number;
		page: ScreenHandlePage;
	} | null>(null);
	let loading = $state(false),
		error = $state(''),
		disposed = false;
	const page = $derived(
		result &&
			result.session === session &&
			session?.status === 'connected' &&
			!session.loading.visible &&
			result.key === key &&
			result.project === session.projectGeneration &&
			result.generation === session.snapshotGeneration
			? result.page
			: null
	);
	function refresh(request: ScreenHandleQuery | null) {
		queue.invalidate();
		const current = session;
		if (
			!request ||
			!visibility.visible ||
			!current ||
			current.status !== 'connected' ||
			current.loading.visible ||
			current.projectGeneration === null
		) {
			result = null;
			loading = false;
			error = '';
			return;
		}
		if (current.hasActiveEditSession || interacting()) {
			loading = false;
			return;
		}
		const generation = current.projectGeneration,
			snapshot = current.snapshotGeneration,
			history = current.currentHistoryStateId,
			requestKey = JSON.stringify(request);
		const valid = () =>
			!disposed &&
			session === current &&
			current.status === 'connected' &&
			!current.loading.visible &&
			!current.hasActiveEditSession &&
			!interacting() &&
			current.projectGeneration === generation &&
			current.snapshotGeneration === snapshot &&
			current.currentHistoryStateId === history &&
			JSON.stringify(query()) === requestKey &&
			visibility.visible;
		loading = true;
		queue.request(
			valid,
			async () => {
				let reply = await invokeScreenEdit(current, {
					kind: 'InspectScreenHandles',
					query: request
				});
				if (reply.kind !== 'ScreenHandles') return reply;
				const page = { ...reply.page, candidates: [...reply.page.candidates] };
				const cursors = new Set<string>();
				while (page.next && valid()) {
					if (cursors.has(page.next) || page.candidates.length > 512)
						throw new Error('Screen handle paging exceeded the visible editor bound');
					cursors.add(page.next);
					reply = await invokeScreenEdit(current, {
						kind: 'InspectScreenHandles',
						query: { ...request, cursor: page.next }
					});
					if (reply.kind !== 'ScreenHandles') return reply;
					page.candidates.push(...reply.page.candidates);
					page.next = reply.page.next;
				}
				return { kind: 'ScreenHandles' as const, page };
			},
			(reply) => {
				if (reply.kind !== 'ScreenHandles') {
					error = 'Unexpected Screen handle response';
					return;
				}
				result = {
					session: current,
					key: requestKey,
					project: generation,
					generation: snapshot,
					page: reply.page
				};
				error = '';
			},
			(failure) => {
				if (valid()) error = String(failure);
			},
			() => {
				loading = false;
			}
		);
	}
	observeEditor({
		query,
		refresh,
		interacting,
		topics: ['mapgyver.screens'],
		owners: () => page?.candidates.map((candidate) => candidate.member.reference.surface) ?? []
	});
	onDestroy(() => {
		disposed = true;
		queue.invalidate();
	});
	const candidates = $derived.by(() => {
		if (!page?.document_matches || !result) return [];
		const parameters = projection.read(result.generation, page.at);
		return page.candidates.map((candidate) => {
			const point = parameters.value(candidate.parameter, 'vec2', [
				candidate.point.x,
				candidate.point.y
			]);
			return { ...candidate, point: { x: point[0], y: point[1] } };
		});
	});
	return {
		get page() {
			return page;
		},
		get candidates() {
			return candidates;
		},
		get loading() {
			return loading;
		},
		get error() {
			return error;
		}
	};
}
