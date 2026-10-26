import type { GraphWorldBounds } from 'golden_graph_ui';
import type { ValueLayout } from './types';
export const SPATIALIZER_MODULE_TYPE = 'spatializer_module';
export const SOURCE_LIST_TYPE = 'spatializer_source_list';
export const TARGET_LIST_TYPE = 'spatializer_target_list';
export const SOURCE_TYPE = 'spatializer_source';
export const TARGET_TYPE = 'spatializer_target';
export const POSITION_2D_DECL_ID = 'position_2d';
export const POSITION_3D_DECL_ID = 'position_3d';
export const RADIUS_DECL_ID = 'radius';
export const FREEZE_RADIUS_DECL_ID = 'freeze_radius';
export const VALUE_TARGET_DECL_PREFIX = 'spatializer_target';
export const VALUE_SOURCE_DECL_PREFIX = 'spatializer_source';
export const VALUE_LAYOUT_DECL_ID = 'value_layout';
export const VALUE_LAYOUT_SOURCE_CENTRIC: ValueLayout = 'sourceCentric';
export const VALUE_LAYOUT_TARGET_CENTRIC: ValueLayout = 'targetCentric';
export const MODE_VORONOI = 'voronoi';
export const MODE_TARGET_RADIUS = 'targetRadius';
export const MODE_SOURCE_RADIUS = 'sourceRadius';
export const MODE_OVERLAP = 'overlap';
export const VORONOI_TIE_EPSILON = 1.0e-9;
export const VORONOI_EDGE_EPSILON = 1.0e-6;
export const DEBUG_WEIGHT_EPSILON = 0.000001;
export const HIGHLIGHT_FADE_MS = 220;
export const CAMERA_PERSIST_DELAY_MS = 150;
export const SPATIALIZER_UNIT_REM = 4;
// Home must fit wide endpoint maps; controls retain their screen-space hit sizes.
export const SPATIALIZER_MIN_ZOOM = 0.001;
export const DEFAULT_SPATIAL_BOUNDS: GraphWorldBounds = { left: -5, top: -5, right: 5, bottom: 5 };
export const TARGET_ZONE_PADDING_UNITS = 1;
export const ENDPOINT_BOUNDS_PADDING_UNITS = 1;
export const MIN_INSPECTOR_WIDTH = 160;
export const MAX_INSPECTOR_WIDTH = 520;
export const DEFAULT_INSPECTOR_WIDTH = 272;
export const ENDPOINT_PALETTE = [
	'#48cae4',
	'#f72585',
	'#80ed99',
	'#ffd166',
	'#90be6d',
	'#f9844a',
	'#b8c0ff',
	'#43aa8b'
];
