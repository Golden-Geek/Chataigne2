use crate::plan::Pattern;

#[repr(C)]
pub(super) struct PatternParameters {
    kind: u32,
    mapped: u32,
    cells: [u32; 2],
    width: f32,
    reserved: [u32; 3],
    colors: [[f32; 4]; 8],
}

impl PatternParameters {
    pub(super) fn new(pattern: Pattern, mapped: bool) -> Self {
        match pattern {
            Pattern::Grid(grid) => {
                let mut colors = [[0.0; 4]; 8];
                colors[0] = grid.foreground().components();
                colors[1] = grid.background().components();
                Self {
                    kind: 0,
                    mapped: u32::from(mapped),
                    cells: grid.cells(),
                    width: grid.width(),
                    reserved: [0; 3],
                    colors,
                }
            }
            Pattern::Bars(colors) => Self {
                kind: 1,
                mapped: u32::from(mapped),
                cells: [0; 2],
                width: 0.0,
                reserved: [0; 3],
                colors: colors.map(|color| color.components()),
            },
        }
    }
}

const _: () = assert!(size_of::<PatternParameters>() == 160);
