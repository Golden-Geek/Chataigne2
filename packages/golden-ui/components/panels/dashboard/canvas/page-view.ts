import type { DashboardPersistedPageView } from './contracts';

export const minPageZoom = 0.25;

export const maxPageZoom = 4;

export const clampPageZoom = (zoom: number): number =>
	Math.min(maxPageZoom, Math.max(minPageZoom, zoom));

export const sanitizePersistedPageView = (
	candidate: DashboardPersistedPageView | null | undefined
): DashboardPersistedPageView => {
	const panX = Number(candidate?.panX);
	const panY = Number(candidate?.panY);
	const zoom = Number(candidate?.zoom);
	return {
		panX: Number.isFinite(panX) ? panX : 0,
		panY: Number.isFinite(panY) ? panY : 0,
		zoom: clampPageZoom(Number.isFinite(zoom) && zoom > 0 ? zoom : 1)
	};
};

export const samePersistedPageView = (
	left: DashboardPersistedPageView | null | undefined,
	right: DashboardPersistedPageView | null | undefined
): boolean => {
	return (
		(left?.panX ?? 0) === (right?.panX ?? 0) &&
		(left?.panY ?? 0) === (right?.panY ?? 0) &&
		(left?.zoom ?? 1) === (right?.zoom ?? 1)
	);
};
