use std::cell::{Cell, RefCell};

use ratatui::{
    buffer::Buffer,
    layout::{Alignment, Rect},
    widgets::Widget,
};

pub fn add(left: u64, right: u64) -> u64 {
    left + right
}
pub struct GridDimension {
    ///The minimum dimension.
    min: u16,
    //The weight for allocating the remaining space.
    weight: u16,
}

//There is 1 more grid point then cell because
//of left and right borders.
struct GridPoint {
    //If there is a widget that spans this grid point
    //it is occluded.
    occluded: bool,
}
pub struct GridLayout {
    columns: Vec<GridDimension>,
    rows: Vec<GridDimension>,
    //This holds the upper left corner and the dimensions of the
    //cells that a widget spans.
    widget_locations: Vec<Rect>,
    //This uses a refcell to enable caching the computed layout.
    edge_layout_x: RefCell<Vec<u16>>,
    edge_layout_y: RefCell<Vec<u16>>,
    grid_points: RefCell<Vec<Vec<GridPoint>>>,
    prior_area: Cell<Rect>,
    dirty_bit: Cell<bool>,
}

fn layout_grid_dim(dims: &Vec<GridDimension>, target: &mut Vec<u16>, start: u16, length: u16) {
    target.clear();
    let mut next_fixed = start;
    for i in 0..dims.len() {
        target.push(next_fixed);
        //There is a +1 for the border
        next_fixed = 1 + dims[i].min;
    }
    target.push(next_fixed); // This is for the right border.
    next_fixed += 1;
    if (next_fixed - start) >= length {
        return;
    }
    let remaining_space = length - (next_fixed - start);

    let total_weight = dims.iter().map(|dim| dim.weight as u32).sum::<u32>();
}

impl GridLayout {
    fn compute_layout(&self, area: Rect) {
        self.dirty_bit.set(false);
        self.prior_area.set(area);
        layout_grid_dim(
            &self.rows,
            &mut self.edge_layout_x.borrow_mut(),
            area.x,
            area.width,
        );
        layout_grid_dim(
            &self.columns,
            &mut self.edge_layout_y.borrow_mut(),
            area.y,
            area.height,
        );
    }
}

impl Widget for &GridLayout {
    fn render(self, area: Rect, buf: &mut Buffer)
    where
        Self: Sized,
    {
        if self.dirty_bit.get() || area != self.prior_area.get() {
            self.compute_layout(area);
        }
        //TODO!
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
