use std::{
    cell::{RefCell},
    collections::BinaryHeap,
};

use ratatui::{buffer::{Buffer, Cell}, layout::Rect, symbols::border, widgets::Widget};

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
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct GridPoint {
    //If there is a widget that spans this grid point
    //it is occluded.
    visible: bool,
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
    //Fully qualified to not conflict with Ratatui cell.
    prior_area: std::cell::Cell<Rect>,
    dirty_bit: std::cell::Cell<bool>,
    border_set: border::Set,
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct WeightItem {
    pub weight: i32,
    pub index: usize,
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
    //If the entire grid is taken up by borders, just return.
    if (next_fixed - start) >= length {
        return;
    }
    //This bit allocates the remaining space by tracking the difference between the ideal allocation
    //and the actual allocation. Due to fractions, matching the ideal allocation may be impossible.
    //This uses a priority queue to get as close as possible.
    let remaining_space = (length - (next_fixed - start)) as i32;
    let total_weight = dims.iter().map(|dim| dim.weight as i32).sum::<i32>();
    let mut weights_heap: BinaryHeap<WeightItem> = BinaryHeap::new();
    for (i, weight) in dims.iter().map(|dim| dim.weight as i32).enumerate() {
        weights_heap.push(WeightItem {
            //Multiply by remaining space to make sure weights are greater than remaining space.
            weight: weight * remaining_space,
            index: i,
        });
    }
    let mut adjustment = vec![0; dims.len()];
    let mut remaining_allocation = remaining_space;
    while remaining_allocation > 0 {
        let Some(mut biggest) = weights_heap.pop() else {
            return;
        };
        //Since each weight was multiplied by remaining_space, there is now total_weight*remaining_space weight.
        //So since there are remaining_space allocations each allocation costs total_weight
        if biggest.weight > total_weight {
            //First, do all of the positive allocations using division to be fast
            let amount = biggest.weight / total_weight;
            remaining_allocation -= amount;
            adjustment[biggest.index] += amount;
            biggest.weight -= total_weight * amount;
        } else {
            //This allocates the remaining pixels
            biggest.weight -= total_weight;
            adjustment[biggest.index] += 1;
            remaining_allocation -= 1;
        }
        weights_heap.push(biggest);
    }
    assert!(remaining_allocation == 0);
    let mut acc = 0;
    for i in 0..adjustment.len() {
        acc += adjustment[i];
        target[i] += acc as u16;
    }
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
        let grid_points = &mut *self.grid_points.borrow_mut();

        let edge_layout_x = &*self.edge_layout_x.borrow();
        let edge_layout_y = &*self.edge_layout_y.borrow();
        *grid_points =
            vec![vec![GridPoint { visible: true }; edge_layout_y.len()]; edge_layout_x.len()];
        for location in &self.widget_locations {
            let location = location.intersection(Rect {
                x: 0,
                y: 0,
                width: edge_layout_x.len() as u16,
                height: edge_layout_y.len() as u16,
            });
            if location.right() <= 1 || location.bottom() <= 1 {
                continue;
            }
            for i in (location.x + 1)..(location.right() - 1) {
                for j in (location.y + 1)..(location.bottom() - 1) {
                    grid_points[i as usize][j as usize].visible = false;
                }
            }
        }
    }

    fn draw_edges(&self, area: Rect, buf: &mut Buffer) {
        let edge_layout_x = &*self.edge_layout_x.borrow();
        let edge_layout_y = &*self.edge_layout_y.borrow();
        let grid_points = &*self.grid_points.borrow();
        //Draw the horizontal lines
        for i in 0..edge_layout_x.len() - 1 {
            for j in 0..edge_layout_y.len() {
                if grid_points[i][j].visible && grid_points[i+1][j].visible {
                    let y = edge_layout_y[j];
                    for x in (edge_layout_x[i] + 1)..(edge_layout_x[i+1]) {
                        buf[(x, y)] = Cell::new(self.border_set.horizontal_top);
                    }
                }
            }
        }
        //Draw the vertical lines
        for i in 0..edge_layout_x.len() {
            for j in 0..edge_layout_y.len() - 1 {
                if grid_points[i][j].visible && grid_points[i][j+1].visible {
                    let x = edge_layout_x[i];
                    for y in (edge_layout_y[j] + 1)..(edge_layout_y[j+1]) {
                        buf[(x, y)] = Cell::new(self.border_set.vertical_left);
                    }
                }
            }
        }
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
        self.draw_edges(area, buf);
        //TODO!
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_test() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 20));
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
