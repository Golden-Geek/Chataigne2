import type { UiNodeDto } from '../../../types';

const normalizeRouteBasePath = (routeBasePath: string): string => {
	const trimmed = routeBasePath.trim();
	if (trimmed.length === 0) {
		return '/dashboard';
	}
	if (trimmed.startsWith('/')) {
		return trimmed.replace(/\/+$/, '') || '/dashboard';
	}
	return `/${trimmed.replace(/\/+$/, '')}`;
};

const basePageSegmentFor = (page: UiNodeDto): string => {
	const normalized = normalizeDashboardRouteLabel(page.meta.label);
	return normalized.length > 0 ? normalized : 'page';
};

const normalizeDashboardRouteLabel = (label: string): string => {
	const normalized = label
		.normalize('NFKD')
		.replace(/[\u0300-\u036f]/g, '')
		.toLowerCase()
		.trim()
		.replace(/[^a-z0-9]+/g, '-')
		.replace(/^-+|-+$/g, '');
	return normalized;
};

export const getDashboardPageRouteSegment = (page: UiNodeDto, pages: UiNodeDto[]): string => {
	const baseSegment = basePageSegmentFor(page);
	const matchingBaseSegments = pages.filter(
		(candidate) => basePageSegmentFor(candidate) === baseSegment
	);
	return matchingBaseSegments.length === 1 ? baseSegment : `${baseSegment}-${page.node_id}`;
};

export const findDashboardPageByRouteSegment = (
	pages: UiNodeDto[],
	pageSegment: string | null | undefined
): UiNodeDto | null => {
	const normalizedSegment = pageSegment?.trim().toLowerCase() ?? '';
	if (normalizedSegment.length === 0) {
		return null;
	}
	const normalizedLabelSegment = normalizeDashboardRouteLabel(pageSegment ?? '');

	for (const page of pages) {
		if (getDashboardPageRouteSegment(page, pages) === normalizedSegment) {
			return page;
		}
	}

	const baseSegmentMatches = pages.filter(
		(candidate) =>
			basePageSegmentFor(candidate) === normalizedSegment ||
			basePageSegmentFor(candidate) === normalizedLabelSegment
	);
	if (baseSegmentMatches.length > 0) {
		return baseSegmentMatches[0];
	}

	return pages.find((candidate) => String(candidate.node_id) === normalizedSegment) ?? null;
};

export const getDashboardPageRouteHref = (
	page: UiNodeDto,
	pages: UiNodeDto[],
	options: {
		routeBasePath?: string;
		hidePages?: boolean;
	} = {}
): string => {
	const basePath = normalizeRouteBasePath(options.routeBasePath ?? '/dashboard');
	const pagePath = `${basePath}/${encodeURIComponent(getDashboardPageRouteSegment(page, pages))}`;
	return options.hidePages ? `${pagePath}?hidePages` : pagePath;
};
