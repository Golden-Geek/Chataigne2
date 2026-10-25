import { tick } from 'svelte';
import type DashboardCanvas from '../../../components/panels/dashboard/DashboardCanvas.svelte';
import type { WorkbenchSession } from '../../../store/workbench.svelte';
import type { DashboardPersistedPageView } from '../../../components/panels/dashboard/canvas/contracts';
import { executeCommand } from '../../../store/commands.svelte';
import { getDirectParamNode } from '../../../components/panels/dashboard/dashboard-model';
import {
	ensure,
	nextFrame,
	measureDashboardAction,
	type DashboardActionStep
} from './dashboard-measure';
import type { dashboardData } from './dashboard-data';
import type { dashboardHierarchy } from './dashboard-hierarchy';

export interface DashboardTimingContext {
	canvas(): ReturnType<typeof DashboardCanvas>;
	data(): ReturnType<typeof dashboardData>;
	session: WorkbenchSession;
	setView(value: DashboardPersistedPageView): void;
	resetEdits(): void;
}
const shell = (id: number) =>
	document.querySelector<HTMLElement>(`.dashboard-widget-shell[data-node-id="${id}"]`);
const contentBounds = (element: Element) => {
	if (getComputedStyle(element).visibility === 'hidden') return null;
	const box = element.getBoundingClientRect();
	let left = box.left,
		right = box.right,
		top = box.top,
		bottom = box.bottom;
	for (
		let parent = element.parentElement;
		parent && !parent.classList.contains('dashboard-page-frame');
		parent = parent.parentElement
	) {
		const style = getComputedStyle(parent),
			rect = parent.getBoundingClientRect();
		const sx = rect.width / Math.max(1, parent.offsetWidth),
			sy = rect.height / Math.max(1, parent.offsetHeight);
		if (style.overflowX !== 'visible') {
			left = Math.max(left, rect.left + parent.clientLeft * sx);
			right = Math.min(right, rect.left + (parent.clientLeft + parent.clientWidth) * sx);
		}
		if (style.overflowY !== 'visible') {
			top = Math.max(top, rect.top + parent.clientTop * sy);
			bottom = Math.min(bottom, rect.top + (parent.clientTop + parent.clientHeight) * sy);
		}
	}
	return right > left && bottom > top
		? { left, right, top, bottom, width: right - left, height: bottom - top }
		: null;
};
const visible = (element: Element | null) => {
	if (!element) return false;
	const r = contentBounds(element),
		clip = document.querySelector('.dashboard-page-scene')!.getBoundingClientRect();
	return (
		!!r && r.right > clip.left && r.left < clip.right && r.bottom > clip.top && r.top < clip.bottom
	);
};
export function createDashboardTiming(context: DashboardTimingContext) {
	let profile = 'sparse',
		tier = 0;
	let containers: ReturnType<typeof dashboardHierarchy>['containers'] = [];
	const canvas = context.canvas,
		session = context.session,
		data = context.data;
	const graph = () => session.graph.state;
	const frameElement = () => document.querySelector<HTMLElement>('.dashboard-page-frame')!;
	const field = (id: number, key: string) => {
		const node = getDirectParamNode(graph(), graph().nodesById.get(id)!, key);
		ensure(node?.data.kind === 'parameter', `Missing ${key} on ${id}`);
		return node.data.param.value;
	};
	const idle = () =>
		!canvas().navigationState().animating &&
		session.editOperations.size === 0 &&
		!session.hasActiveEditSession;
	const navigationIdle = () => !canvas().navigationState().animating;
	const command = (id: 'view.home' | 'view.frame') => executeCommand(id, { source: 'api' });
	const centerError = (element: Element) => {
		const r = element.classList.contains('dashboard-page-frame')
				? element.getBoundingClientRect()
				: contentBounds(element),
			v = document.querySelector('.dashboard-page-viewport')!.getBoundingClientRect();
		ensure(r, 'Framed source is fully clipped');
		return Math.max(
			Math.abs(r.left + r.width / 2 - v.left - v.width / 2),
			Math.abs(r.top + r.height / 2 - v.top - v.height / 2)
		);
	};
	const paintedContentVisible = () =>
		[...document.querySelectorAll('.dashboard-widget-shell, .dashboard-free-overview')].some(
			visible
		);
	const navigationFeedback = (before: string) =>
		frameElement().style.transform !== before && paintedContentVisible();
	const actions = () =>
		profile === 'sparse'
			? [
					'select',
					'rectangle',
					'move-single',
					'move-four',
					'resize',
					'pan',
					'zoom',
					'frame-far',
					'home'
				]
			: profile.startsWith('dense')
				? ['pick', 'bulk-select', 'bulk-frame']
				: profile.startsWith('minimum')
					? ['pick', 'pan']
					: profile === 'deep'
						? ['deep-frame']
						: ['free', 'horizontal', 'vertical', 'grid', 'tabs', 'accordion'].flatMap((layout) => [
								`discover-${layout}`,
								`frame-${layout}`
							]);
	let selectedTarget = 0;
	const prepare = async (action: string, index: number) => {
		session.editOperations.reset();
		await tick();
		await nextFrame();
		context.resetEdits();
		document.querySelector<HTMLElement>('.dashboard-page-viewport')!.focus();
		session.selectNodes([], 'REPLACE');
		context.setView({
			panX: action === 'home' ? 64 : 0,
			panY: action === 'home' ? 32 : 0,
			zoom: profile.startsWith('minimum') ? 0.25 : action === 'home' ? 0.5 : 1
		});
		if (action === 'move-four')
			session.selectNodes(
				[0, 1, 40, 41].map((i) => data().ids[i]),
				'REPLACE'
			);
		else if (action.startsWith('move') || action === 'resize')
			session.selectNodes([data().ids[0]], 'REPLACE');
		else if (action === 'bulk-frame') session.selectNodes(data().ids, 'REPLACE');
		else if (action === 'frame-far' || action === 'deep-frame') {
			selectedTarget = data().ids[data().ids.length - 1 - (index % 2)];
			session.selectNodes([selectedTarget], 'REPLACE');
		} else if (action.startsWith('frame-') || action.startsWith('discover-')) {
			const family = action.slice(action.indexOf('-') + 1);
			const container = containers.find((entry) => entry.layout === family)!;
			selectedTarget = container.ids.at(-1)!;
			if (action.startsWith('frame-')) session.selectNodes([selectedTarget], 'REPLACE');
			else canvas().chooseSurfaceChild(container.id, container.ids[0]);
		}
		await tick();
		await nextFrame();
		await tick();
		await nextFrame();
		ensure(navigationIdle(), 'Preparation left a camera animation active');
	};
	const transform = async (action: string, index: number): Promise<DashboardActionStep> => {
		const ids = action === 'move-four' ? [0, 1, 40, 41].map((i) => data().ids[i]) : [data().ids[0]];
		const before = ids.map((id) => {
			const element = shell(id);
			ensure(element, `Transform source ${id} is not mounted`);
			const position = field(id, 'position'),
				width = field(id, 'width'),
				height = field(id, 'height');
			ensure(
				position.kind === 'vec2' && width.kind === 'css_value' && height.kind === 'css_value',
				'Invalid transform source'
			);
			return {
				id,
				box: element.getBoundingClientRect(),
				position: [...position.value],
				width: width.value,
				height: height.value
			};
		});
		const box = before[0].box,
			origin = { x: box.x + box.width / 2, y: box.y + box.height / 2 };
		const kind = action === 'resize' ? 'resize' : 'move';
		ensure(
			await canvas().editWidgetLayout(
				ids[0],
				kind === 'move'
					? { kind, origin }
					: { kind, origin, edges: { left: false, top: false, right: true, bottom: true } }
			),
			'Transform did not start'
		);
		const delta = index % 2 ? -24 : 24;
		ensure(
			await canvas().editWidgetLayout(ids[0], {
				kind: 'update',
				point: { x: origin.x + delta, y: origin.y + delta }
			}),
			'Transform did not update'
		);
		return {
			complete: Promise.resolve(),
			feedback: () =>
				before.every(({ id, box }) => {
					const r = shell(id)?.getBoundingClientRect();
					return !!r && Math.abs(kind === 'move' ? r.x - box.x : r.width - box.width) > 8;
				}),
			afterFeedback: () => canvas().editWidgetLayout(ids[0], { kind: 'finish' }),
			correct: () => {
				const surface = shell(ids[0])!.parentElement!.closest<HTMLElement>('.dashboard-surface')!;
				const scale = surface.getBoundingClientRect().width / surface.clientWidth;
				const rem = parseFloat(getComputedStyle(document.documentElement).fontSize);
				for (const previous of before) {
					const r = shell(previous.id)?.getBoundingClientRect(),
						p = field(previous.id, 'position'),
						w = field(previous.id, 'width'),
						h = field(previous.id, 'height');
					ensure(
						r && p.kind === 'vec2' && w.kind === 'css_value' && h.kind === 'css_value',
						'Transform lost its source'
					);
					const expected = {
						x: previous.box.x + (p.value[0] - previous.position[0]) * scale,
						y: previous.box.y + (p.value[1] - previous.position[1]) * scale,
						width: previous.box.width + (w.value - previous.width) * rem * scale,
						height: previous.box.height + (h.value - previous.height) * rem * scale
					};
					ensure(
						Math.max(
							...(['x', 'y', 'width', 'height'] as const).map((key) =>
								Math.abs(r[key] - expected[key])
							)
						) <= 0.5,
						'Rendered transform differs from acknowledged geometry'
					);
				}
			}
		};
	};
	const start = async (action: string, index: number): Promise<DashboardActionStep> => {
		if (action.startsWith('move') || action === 'resize') return transform(action, index);
		if (action === 'select' || action === 'bulk-select' || action === 'rectangle') {
			const expected =
				action === 'select'
					? [data().ids[index % 2]]
					: action === 'bulk-select'
						? data().ids
						: [0, 1, 40, 41].map((i) => data().ids[i]);
			if (action === 'rectangle') {
				const surface = document.querySelector<HTMLElement>('[data-dashboard-free-total]')!;
				const rect = surface.getBoundingClientRect(),
					sx = rect.width / surface.clientWidth,
					sy = rect.height / surface.clientHeight;
				canvas().selectWidgetsInRect({
					left: rect.left - sx,
					top: rect.top - sy,
					width: 434 * sx,
					height: 194 * sy
				});
			} else session.selectNodes(expected, 'REPLACE');
			const correct = () => {
				const actual = new Set(session.selectedNodesIds);
				ensure(
					actual.size === expected.length && expected.every((id) => actual.has(id)),
					'Logical selection differs'
				);
				ensure(
					expected.slice(-16).some((id) => shell(id)?.classList.contains('selected')),
					'Selection has no rendered control cohort'
				);
			};
			return {
				complete: Promise.resolve(),
				feedback: () => expected.slice(-16).some((id) => shell(id)?.classList.contains('selected')),
				correct
			};
		}
		if (action === 'pick') {
			const id = profile.startsWith('dense') ? data().ids.at(-1)! : data().ids[0];
			const source = profile.startsWith('dense') ? data().ids[0] : id;
			const position = field(source, 'position');
			ensure(position.kind === 'vec2', 'Missing pick point');
			const layout = document.querySelector<HTMLElement>('[data-dashboard-free-total]')!,
				r = layout.getBoundingClientRect();
			const scale = r.width / layout.clientWidth;
			ensure(
				canvas().activateWidgetAt(
					data().page.node_id,
					r.left + (position.value[0] + 96) * scale,
					r.top + (position.value[1] + 40) * scale
				) === id,
				'Exact foremost picking returned another identity'
			);
			const correct = () => {
				ensure(
					shell(id)?.classList.contains('selected') && visible(shell(id)),
					'Picked control did not render'
				);
				ensure(
					shell(id)!.textContent?.includes(`Widget ${data().ids.indexOf(id)}`),
					'Picked control label differs'
				);
			};
			return {
				complete: Promise.resolve(),
				feedback: () => !!shell(id)?.classList.contains('selected') && visible(shell(id)),
				correct
			};
		}
		if (action === 'pan' || action === 'zoom') {
			const before = frameElement().getBoundingClientRect();
			const zoom = action === 'zoom' ? 0.75 : profile.startsWith('minimum') ? 0.25 : 1;
			context.setView({ panX: action === 'pan' ? 64 : 0, panY: 0, zoom });
			const correct = () => {
				const after = frameElement().getBoundingClientRect();
				ensure(paintedContentVisible(), 'Navigation has no visible scene content');
				ensure(
					action === 'pan'
						? Math.abs(after.x - before.x - 64) <= 0.5
						: Math.abs(after.width / before.width - 0.75) < 0.001,
					'Navigation geometry differs'
				);
			};
			return {
				complete: Promise.resolve(),
				feedback: () =>
					paintedContentVisible() &&
					(Math.abs(frameElement().getBoundingClientRect().x - before.x) > 1 ||
						Math.abs(frameElement().getBoundingClientRect().width - before.width) > 1),
				correct
			};
		}
		if (action.startsWith('discover-')) {
			const container = containers.find((entry) => entry.layout === action.slice(9))!;
			const complete = (async () => {
				if (canvas().browseSurface(container.id)) {
					await tick();
					canvas().searchSurfaceChildren(
						container.id,
						graph().nodesById.get(selectedTarget)!.meta.label
					);
					await tick();
					ensure(
						document.querySelectorAll('.dashboard-child-menu .gc-context-item').length > 0,
						'Browser discovery produced no matching row'
					);
				}
				ensure(
					canvas().chooseSurfaceChild(container.id, selectedTarget),
					'Child discovery refused its source'
				);
			})();
			const correct = () =>
				ensure(
					shell(selectedTarget) && visible(shell(selectedTarget)),
					'Discovered child has no visible control'
				);
			return {
				complete,
				feedback: () => !!shell(selectedTarget) && visible(shell(selectedTarget)),
				correct
			};
		}
		const before = frameElement().style.transform;
		const complete = command(action === 'home' ? 'view.home' : 'view.frame').then((handled) =>
			ensure(handled, 'Navigation command was not handled')
		);
		const correct = () => {
			if (action === 'home')
				ensure(centerError(frameElement()) <= 0.5, 'Home did not center the authored page');
			else {
				const id = action === 'bulk-frame' ? data().ids.at(-1)! : selectedTarget;
				ensure(shell(id) && visible(shell(id)), 'Framed source did not render');
				ensure(centerError(shell(id)!) <= 0.5, 'Frame did not center the selected geometry');
			}
		};
		return { complete, feedback: () => navigationFeedback(before), correct };
	};
	return {
		configure(size: number, kind: string, branches: typeof containers = []) {
			tier = size;
			profile = kind;
			containers = branches;
		},
		actions,
		run: (action: string) =>
			measureDashboardAction({
				name: action,
				profile,
				size: tier,
				prepare: (index) => prepare(action, index),
				start: (index) => start(action, index),
				idle,
				diagnostics: () => {
					const element = shell(selectedTarget);
					const ancestors = [];
					for (
						let current: HTMLElement | null = element;
						current && current !== document.body;
						current = current.parentElement
					) {
						const style = getComputedStyle(current);
						ancestors.push({
							classes: current.className,
							box: current.getBoundingClientRect().toJSON(),
							clientWidth: current.clientWidth,
							clientHeight: current.clientHeight,
							scrollTop: current.scrollTop,
							scrollHeight: current.scrollHeight,
							overflowX: style.overflowX,
							overflowY: style.overflowY,
							rows: style.gridTemplateRows,
							columns: style.gridTemplateColumns
						});
					}
					return {
						selectedTarget,
						selected: session.selectedNodesIds.slice(0, 8),
						selectedCount: session.selectedNodesIds.length,
						ancestors
					};
				}
			})
	};
}
