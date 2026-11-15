export interface CurveCanvasTheme {
	canvas_bg: string;
	canvas_wrap_bg: string;
	canvas_wrap_border: string;
	line_idle: string;
	point_idle: string;
	fill_rgb: string;
	grid_line_rgb: string;
	grid_label_rgb: string;
	border_unbounded: string;
	border_bounded: string;
	empty_text: string;
	plot_fill: string;
	plot_shadow: string;
	plot_shadow_fill: string;
	hover_segment_underlay: string;
	hover_segment: string;
	selected_segment_underlay: string;
	selected_segment: string;
	handle_line: string;
	handle_line_aligned: string;
	handle_line_unlocked: string;
	handle_active: string;
	handle_hover: string;
	handle_out: string;
	handle_in: string;
	handle_stroke: string;
	handle_anchor_aligned: string;
	handle_anchor_unlocked: string;
	key_selected: string;
	key_hover: string;
	key_stroke: string;
	key_guide: string;
	draw_path_stroke: string;
	draw_path_point: string;
	crosshair: string;
	selection_fill: string;
	selection_stroke: string;
	bounds_readout: string;
}

const css_var_or = (
	styles: Pick<CSSStyleDeclaration, 'getPropertyValue'>,
	variable_name: string,
	fallback: string
): string => {
	const value = styles.getPropertyValue(variable_name).trim();
	return value.length > 0 ? value : fallback;
};

export const initialCurveCanvasTheme: Readonly<CurveCanvasTheme> = {
	canvas_bg: '#000000',
	canvas_wrap_bg: 'transparent',
	canvas_wrap_border: 'transparent',
	line_idle: '#9aa1ac',
	point_idle: '#b1bac9',
	fill_rgb: '80, 162, 255',
	grid_line_rgb: '164, 174, 191',
	grid_label_rgb: '186, 195, 210',
	border_unbounded: 'rgba(138, 147, 161, 0.46)',
	border_bounded: 'rgba(185, 199, 219, 0.82)',
	empty_text: 'rgba(192, 206, 232, 0.64)',
	plot_fill: 'rgba(0, 0, 0, 0.2)',
	plot_shadow: 'rgba(0, 0, 0, 0.5)',
	plot_shadow_fill: '#000000',
	hover_segment_underlay: 'rgba(92, 186, 255, 0.24)',
	hover_segment: 'rgba(146, 214, 255, 0.74)',
	selected_segment_underlay: 'rgba(92, 241, 170, 0.34)',
	selected_segment: 'rgba(150, 255, 203, 0.96)',
	handle_line: 'rgba(222, 232, 251, 0.32)',
	handle_line_aligned: 'rgba(120, 234, 198, 0.46)',
	handle_line_unlocked: 'rgba(255, 177, 118, 0.42)',
	handle_active: 'rgba(255, 224, 130, 0.98)',
	handle_hover: 'rgba(255, 199, 138, 0.95)',
	handle_out: 'rgba(172, 235, 220, 0.9)',
	handle_in: 'rgba(164, 205, 245, 0.9)',
	handle_stroke: 'rgba(10, 16, 24, 0.95)',
	handle_anchor_aligned: 'rgba(141, 245, 205, 0.94)',
	handle_anchor_unlocked: 'rgba(255, 183, 125, 0.94)',
	key_selected: 'rgba(255, 229, 112, 0.98)',
	key_hover: 'rgba(255, 182, 96, 0.95)',
	key_stroke: 'rgba(8, 12, 18, 0.9)',
	key_guide: 'rgba(255, 226, 117, 0.35)',
	draw_path_stroke: 'rgba(255, 140, 98, 0.96)',
	draw_path_point: 'rgba(255, 198, 122, 0.98)',
	crosshair: 'rgba(217, 231, 255, 0)',
	selection_fill: 'rgba(114, 210, 255, 0.14)',
	selection_stroke: 'rgba(158, 228, 255, 0.9)',
	bounds_readout: 'rgba(198, 208, 224, 0.86)'
};

export function readCurveCanvasTheme(
	styles: Pick<CSSStyleDeclaration, 'getPropertyValue'>
): CurveCanvasTheme {
	return {
		canvas_bg: css_var_or(styles, '--gc-color-curve-canvas-bg', '#171b20'),
		canvas_wrap_bg: css_var_or(styles, '--gc-color-curve-canvas-wrap-bg', 'transparent'),
		canvas_wrap_border: css_var_or(styles, '--gc-color-curve-canvas-wrap-border', 'transparent'),
		line_idle: css_var_or(styles, '--gc-color-curve-line-idle', '#9aa1ac'),
		point_idle: css_var_or(styles, '--gc-color-curve-point-idle', '#b1bac9'),
		fill_rgb: css_var_or(styles, '--gc-color-curve-fill-rgb', '80, 162, 255'),
		grid_line_rgb: css_var_or(styles, '--gc-color-curve-grid-line-rgb', '164, 174, 191'),
		grid_label_rgb: css_var_or(styles, '--gc-color-curve-grid-label-rgb', '186, 195, 210'),
		border_unbounded: css_var_or(
			styles,
			'--gc-color-curve-border-unbounded',
			'rgba(138, 147, 161, 0.46)'
		),
		border_bounded: css_var_or(
			styles,
			'--gc-color-curve-border-bounded',
			'rgba(185, 199, 219, 0.82)'
		),
		empty_text: css_var_or(styles, '--gc-color-curve-empty-text', 'rgba(192, 206, 232, 0.64)'),
		plot_fill: css_var_or(styles, '--gc-color-curve-plot-fill', 'rgba(0, 0, 0, 0.2)'),
		plot_shadow: css_var_or(styles, '--gc-color-curve-plot-shadow', 'rgba(0, 0, 0, 0.5)'),
		plot_shadow_fill: css_var_or(styles, '--gc-color-curve-plot-shadow-fill', '#000000'),
		hover_segment_underlay: css_var_or(
			styles,
			'--gc-color-curve-hover-segment-underlay',
			'rgba(92, 186, 255, 0.24)'
		),
		hover_segment: css_var_or(
			styles,
			'--gc-color-curve-hover-segment',
			'rgba(146, 214, 255, 0.74)'
		),
		selected_segment_underlay: css_var_or(
			styles,
			'--gc-color-curve-selected-segment-underlay',
			'rgba(92, 241, 170, 0.34)'
		),
		selected_segment: css_var_or(
			styles,
			'--gc-color-curve-selected-segment',
			'rgba(150, 255, 203, 0.96)'
		),
		handle_line: css_var_or(styles, '--gc-color-curve-handle-line', 'rgba(222, 232, 251, 0.32)'),
		handle_line_aligned: css_var_or(
			styles,
			'--gc-color-curve-handle-line-aligned',
			'rgba(120, 234, 198, 0.46)'
		),
		handle_line_unlocked: css_var_or(
			styles,
			'--gc-color-curve-handle-line-unlocked',
			'rgba(255, 177, 118, 0.42)'
		),
		handle_active: css_var_or(
			styles,
			'--gc-color-curve-handle-active',
			'rgba(255, 224, 130, 0.98)'
		),
		handle_hover: css_var_or(styles, '--gc-color-curve-handle-hover', 'rgba(255, 199, 138, 0.95)'),
		handle_out: css_var_or(styles, '--gc-color-curve-handle-out', 'rgba(172, 235, 220, 0.9)'),
		handle_in: css_var_or(styles, '--gc-color-curve-handle-in', 'rgba(164, 205, 245, 0.9)'),
		handle_stroke: css_var_or(styles, '--gc-color-curve-handle-stroke', 'rgba(10, 16, 24, 0.95)'),
		handle_anchor_aligned: css_var_or(
			styles,
			'--gc-color-curve-handle-anchor-aligned',
			'rgba(141, 245, 205, 0.94)'
		),
		handle_anchor_unlocked: css_var_or(
			styles,
			'--gc-color-curve-handle-anchor-unlocked',
			'rgba(255, 183, 125, 0.94)'
		),
		key_selected: css_var_or(styles, '--gc-color-curve-key-selected', 'rgba(255, 229, 112, 0.98)'),
		key_hover: css_var_or(styles, '--gc-color-curve-key-hover', 'rgba(255, 182, 96, 0.95)'),
		key_stroke: css_var_or(styles, '--gc-color-curve-key-stroke', 'rgba(8, 12, 18, 0.9)'),
		key_guide: css_var_or(styles, '--gc-color-curve-key-guide', 'rgba(255, 226, 117, 0.35)'),
		draw_path_stroke: css_var_or(
			styles,
			'--gc-color-curve-draw-path-stroke',
			'rgba(255, 140, 98, 0.96)'
		),
		draw_path_point: css_var_or(
			styles,
			'--gc-color-curve-draw-path-point',
			'rgba(255, 198, 122, 0.98)'
		),
		crosshair: css_var_or(styles, '--gc-color-curve-crosshair', 'rgba(217, 231, 255, 0)'),
		selection_fill: css_var_or(
			styles,
			'--gc-color-curve-selection-fill',
			'rgba(114, 210, 255, 0.14)'
		),
		selection_stroke: css_var_or(
			styles,
			'--gc-color-curve-selection-stroke',
			'rgba(158, 228, 255, 0.9)'
		),
		bounds_readout: css_var_or(
			styles,
			'--gc-color-curve-bounds-readout',
			'rgba(198, 208, 224, 0.86)'
		)
	};
}
