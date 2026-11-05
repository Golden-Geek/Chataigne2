import type { SpatialResizeHandle } from '../media/generated/SpatialResizeHandle';

export const SPATIAL_RESIZE_HANDLES = [
	{ name: 'TopLeft', x: 0, y: 0, cursor: 'nwse-resize' },
	{ name: 'Top', x: 0.5, y: 0, cursor: 'ns-resize' },
	{ name: 'TopRight', x: 1, y: 0, cursor: 'nesw-resize' },
	{ name: 'Right', x: 1, y: 0.5, cursor: 'ew-resize' },
	{ name: 'BottomRight', x: 1, y: 1, cursor: 'nwse-resize' },
	{ name: 'Bottom', x: 0.5, y: 1, cursor: 'ns-resize' },
	{ name: 'BottomLeft', x: 0, y: 1, cursor: 'nesw-resize' },
	{ name: 'Left', x: 0, y: 0.5, cursor: 'ew-resize' }
] satisfies { name: SpatialResizeHandle; x: number; y: number; cursor: string }[];
