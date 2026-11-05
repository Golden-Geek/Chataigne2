import type { PreviewGeometry, PreviewSession, PreviewSessionState } from './session';
import type { CssRect } from './generated/CssRect';
import { RetirementCovers } from './retirement';
import { RetirementLayer } from './retirement-layer';

const observations = new WeakMap<PreviewSession, LayoutObservation>();
interface View {
	id: string;
	element: HTMLElement;
	latest?: PreviewGeometry;
	nativeRect?: CssRect;
}

// One observation/retirement owner per editor session survives individual component unmounts.
class LayoutObservation {
	private readonly views = new Map<string, View>();
	private readonly targets = new Set<HTMLElement>();
	private readonly covers = new RetirementCovers();
	private layer: RetirementLayer | null = null;
	private everConnected = false;
	private started = false;
	private unsubscribe = () => {};
	private readonly resize = new ResizeObserver(() => this.session.invalidate());
	private readonly mutation = new MutationObserver((records) => {
		if (this.layer && records.every((record) => this.layer!.element.contains(record.target)))
			return;
		this.observeAncestors();
		this.session.invalidate();
	});
	private readonly invalidate = () => this.session.invalidate();

	constructor(private readonly session: PreviewSession) {
		this.mutation.observe(document.documentElement, {
			subtree: true,
			childList: true,
			attributes: true,
			attributeFilter: ['style', 'class', 'hidden', 'open', 'aria-expanded']
		});
		window.addEventListener('resize', this.invalidate);
		window.addEventListener('scroll', this.invalidate, true);
		window.addEventListener('load', this.invalidate, true);
		window.visualViewport?.addEventListener('resize', this.invalidate);
		window.visualViewport?.addEventListener('scroll', this.invalidate);
		document.addEventListener('visibilitychange', this.invalidate);
		document.fonts?.addEventListener('loadingdone', this.invalidate);
		this.unsubscribe = session.subscribe((state) => this.stateChanged(state));
	}

	add(element: HTMLElement, id: string) {
		this.started = true;
		this.views.set(id, { id, element });
		this.observeAncestors();
	}

	measured(id: string, geometry: PreviewGeometry) {
		const view = this.views.get(id);
		if (!view) return;
		if (this.everConnected && JSON.stringify(view.latest) !== JSON.stringify(geometry)) {
			this.protect(view);
			if (geometry.visible) this.covers.protect(id, geometry.clip);
		}
		view.latest = geometry;
		this.renderCovers();
	}

	remove(id: string) {
		const view = this.views.get(id);
		if (!view) return;
		if (
			this.everConnected &&
			!(this.session.state.installed && this.session.state.visibleViews === 0)
		)
			this.protect(view);
		this.views.delete(id);
		this.observeAncestors();
		this.renderCovers();
		this.finishIfUnused();
	}

	private stateChanged(state: PreviewSessionState) {
		this.everConnected ||= state.session !== null;
		if (state.installed) {
			// Only a latest matching native installation/preview-window acceptance releases masks.
			// A Disposed/Disconnected metadata state alone provides no such proof.
			this.covers.clear();
			for (const view of this.views.values())
				view.nativeRect =
					state.presentedViews.includes(view.id) && view.latest?.visible
						? view.latest.clip
						: undefined;
		} else if (this.everConnected) {
			for (const view of this.views.values())
				if (!state.presentedViews.includes(view.id)) this.protect(view);
		}
		this.renderCovers();
		this.finishIfUnused();
	}

	private protect(view: View) {
		if (view.nativeRect) this.covers.protect(view.id, view.nativeRect);
		if (view.latest?.visible) this.covers.protect(view.id, view.latest.clip);
	}

	private renderCovers() {
		const entries = this.covers.entries();
		if (!entries.length) {
			this.layer?.element.remove();
			this.layer = null;
			return;
		}
		if (!this.layer) {
			this.layer = new RetirementLayer();
			document.body.prepend(this.layer.element);
		}
		const apertures: Array<readonly [string, CssRect]> = [];
		for (const view of this.views.values())
			if (
				view.nativeRect &&
				view.latest?.visible &&
				this.session.state.presentedViews.includes(view.id)
			)
				apertures.push([view.id, view.latest.clip]);
		this.layer.update(entries, apertures);
	}

	private finishIfUnused() {
		if (!this.started || this.views.size || this.covers.size) return;
		this.unsubscribe();
		this.resize.disconnect();
		this.mutation.disconnect();
		window.removeEventListener('resize', this.invalidate);
		window.removeEventListener('scroll', this.invalidate, true);
		window.removeEventListener('load', this.invalidate, true);
		window.visualViewport?.removeEventListener('resize', this.invalidate);
		window.visualViewport?.removeEventListener('scroll', this.invalidate);
		document.removeEventListener('visibilitychange', this.invalidate);
		document.fonts?.removeEventListener('loadingdone', this.invalidate);
		observations.delete(this.session);
	}

	private observeAncestors() {
		const current = new Set<HTMLElement>();
		for (const { element } of this.views.values()) {
			for (let ancestor: HTMLElement | null = element; ancestor; ancestor = ancestor.parentElement)
				current.add(ancestor);
		}
		for (const target of this.targets)
			if (!current.has(target)) {
				this.resize.unobserve(target);
				this.targets.delete(target);
			}
		for (const target of current)
			if (!this.targets.has(target)) {
				this.resize.observe(target);
				this.targets.add(target);
			}
	}
}

export function observePreviewLayout(session: PreviewSession, element: HTMLElement, id: string) {
	let observation = observations.get(session);
	if (!observation) {
		observation = new LayoutObservation(session);
		observations.set(session, observation);
	}
	observation.add(element, id);
	return {
		measured: (geometry: PreviewGeometry) => observation.measured(id, geometry),
		dispose: () => observation.remove(id)
	};
}
