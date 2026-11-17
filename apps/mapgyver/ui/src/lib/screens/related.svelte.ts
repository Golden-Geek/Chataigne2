import { onDestroy } from 'svelte';
import { appState, type WorkbenchSession } from 'golden_ui';
import { ReadQueue } from 'golden_ui/store/bounded-read-queue';
import { observeEditor } from 'golden_ui/store/editor-observation.svelte';
import { usePanelVisibility } from 'golden_ui/dockview/panel-visibility.svelte';
import { createEditorParameterProjection } from 'golden_ui/store/editor-parameter-projection.svelte';
import { invokeScreenEdit } from './operations';
import { projectSurface } from './parameter-projection';
import type { RelatedSurfaceQuery } from './generated/RelatedSurfaceQuery';
import type { RelatedSurfacePage } from './generated/RelatedSurfacePage';

/** One bounded metadata reader per panel; peer geometry never owns another preview/source. */
export function createRelatedSurfaces(query: () => RelatedSurfaceQuery | null) {
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
		page: RelatedSurfacePage;
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
			result.project === session.projectGeneration
			? result.page
			: null
	);
	function refresh(request: RelatedSurfaceQuery | null) {
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
		if (current.hasActiveEditSession) {
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
			current.projectGeneration === generation &&
			current.snapshotGeneration === snapshot &&
			current.currentHistoryStateId === history &&
			JSON.stringify(query()) === requestKey &&
			visibility.visible;
		loading = true;
		queue.request(
			valid,
			() => invokeScreenEdit(current, { kind: 'InspectRelatedSurfaces', query: request }),
			(reply) => {
				if (reply.kind !== 'RelatedSurfaces') {
					error = 'Unexpected related Surface response';
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
		topics: ['mapgyver.screens', 'mapgyver.media'],
		owners: () => page?.surfaces.map((entry) => entry.surface.reference.surface) ?? []
	});
	onDestroy(() => {
		disposed = true;
		queue.invalidate();
	});
	const surfaces = $derived.by(() => {
		if (!page?.document_matches || !result) return [];
		const parameters = projection.read(result.generation, page.at);
		return page.surfaces.map((entry) => ({
			...entry,
			surface: projectSurface(entry.surface, entry.parameters, parameters)
		}));
	});
	return {
		get page() {
			return page;
		},
		get surfaces() {
			return surfaces;
		},
		get loading() {
			return loading;
		},
		get error() {
			return error;
		}
	};
}
