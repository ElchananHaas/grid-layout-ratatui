use std::{cell::RefCell, collections::BinaryHeap};

use ratatui::{
    buffer::{Buffer, Cell},
    layout::Rect,
    symbols::{border, line::NORMAL},
    widgets::Widget,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
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
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct WeightItem {
    pub weight: i32,
    pub index: usize,
}
fn layout_grid_dim(dims: &Vec<GridDimension>, target: &mut Vec<u16>, start: u16, length: u16) {
    target.clear();
    let mut sizes: Vec<i32> = Vec::new();
    for i in 0..dims.len() {
        //There is a +1 for the border
        sizes.push(1 + dims[i].min as i32);
    }
    let taken_up: i32 = sizes.iter().sum();
    //Minus 1 for the right border
    let mut allocate = (length as i32) - taken_up - 1;
    let total_weight = dims.iter().map(|dim| dim.weight as i32).sum::<i32>();
    //This bit allocates the remaining space by tracking the difference between the ideal allocation
    //and the actual allocation. Due to fractions, matching the ideal allocation may be impossible.
    //This uses a priority queue to get as close as possible.
    let mut weights_heap: BinaryHeap<WeightItem> = BinaryHeap::new();
    for (i, weight) in dims.iter().map(|dim| dim.weight as i32).enumerate() {
        weights_heap.push(WeightItem {
            //There are (total_weight*allocate) tokens. Each space costs
            //total_weight tokens. The min is already allocated, so subtract it.
            weight: weight * allocate - (total_weight * (dims[i].min as i32)),
            index: i,
        });
    }
    while allocate > 0 {
        let Some(mut biggest) = weights_heap.pop() else {
            return;
        };
        //Since each weight was multiplied by remaining_space, there is now total_weight*remaining_space weight.
        //So since there are remaining_space allocations each allocation costs total_weight
        if biggest.weight > total_weight {
            //First, do all of the positive allocations using division to be fast
            let amount = biggest.weight / total_weight;
            allocate -= amount;
            sizes[biggest.index] += amount;
            biggest.weight -= total_weight * amount;
        } else {
            //This allocates the remaining pixels
            biggest.weight -= total_weight;
            sizes[biggest.index] += 1;
            allocate -= 1;
        }
        weights_heap.push(biggest);
    }
    assert!(allocate <= 0);
    dbg!(&sizes);
    let mut acc = start;
    for i in 0..sizes.len() {
        target.push(acc as u16);
        acc += sizes[i] as u16;
    }
    //For the right border.
    target.push(acc as u16);
}

fn corner_symbol(top: bool, right: bool, bottom: bool, left: bool) -> &'static str {
    match (top, right, bottom, left) {
        (true, true, true, true) => NORMAL.cross,
        (true, true, true, false) => NORMAL.vertical_right,
        (true, true, false, true) => NORMAL.horizontal_up,
        (true, true, false, false) => NORMAL.bottom_left,
        (true, false, true, true) => NORMAL.vertical_left,
        (true, false, true, false) => NORMAL.vertical,
        (true, false, false, true) => NORMAL.bottom_right,
        (true, false, false, false) => &"╵",
        (false, true, true, true) => NORMAL.horizontal_down,
        (false, true, true, false) => NORMAL.top_left,
        (false, true, false, true) => NORMAL.horizontal,
        (false, true, false, false) => &"╶",
        (false, false, true, true) => NORMAL.top_right,
        (false, false, true, false) => &"╷",
        (false, false, false, true) => &"╴",
        (false, false, false, false) => &" ",
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
                if grid_points[i][j].visible && grid_points[i + 1][j].visible {
                    let y = edge_layout_y[j];
                    for x in (edge_layout_x[i] + 1)..(edge_layout_x[i + 1]) {
                        buf.cell_mut((x, y)).map(|c| *c = Cell::new(NORMAL.horizontal));
                    }
                }
            }
        }
        //Draw the vertical lines
        for i in 0..edge_layout_x.len() {
            for j in 0..edge_layout_y.len() - 1 {
                if grid_points[i][j].visible && grid_points[i][j + 1].visible {
                    let x = edge_layout_x[i];
                    for y in (edge_layout_y[j] + 1)..(edge_layout_y[j + 1]) {
                        buf.cell_mut((x, y)).map(|c| *c = Cell::new(NORMAL.vertical));
                    }
                }
            }
        }
    }

    fn draw_corners(&self, area: Rect, buf: &mut Buffer) {
        let edge_layout_x = &*self.edge_layout_x.borrow();
        let edge_layout_y = &*self.edge_layout_y.borrow();
        let grid_points = &*self.grid_points.borrow();
        for i in 0..edge_layout_x.len() {
            for j in 0..edge_layout_y.len() {
                if !grid_points[i][j].visible {
                    continue;
                }
                let top = j > 0 && grid_points[i][j - 1].visible;
                let right = grid_points.get(i + 1).is_some_and(|row| row[j].visible);
                let bottom = grid_points[i].get(j + 1).is_some_and(|point| point.visible);
                let left = i > 0 && grid_points[i - 1][j].visible;
                let symbol = corner_symbol(top, right, bottom, left);
                buf.cell_mut((edge_layout_x[i], edge_layout_y[j])).map(|c| *c = Cell::new(symbol));
            }
        }
    }
    pub fn set_columns(&mut self, columns: Vec<GridDimension>) {
        self.columns = columns;
        self.dirty_bit.set(true);
    }
    pub fn set_rows(&mut self, rows: Vec<GridDimension>) {
        self.rows = rows;
        self.dirty_bit.set(true);
    }
    pub fn add_widget(&mut self, place: Rect) {
        self.widget_locations.push(place);
    }
    pub fn new() -> Self {
        GridLayout {
            columns: vec![],
            rows: vec![],
            widget_locations: vec![],
            edge_layout_x: RefCell::new(Vec::new()),
            edge_layout_y: RefCell::new(Vec::new()),
            grid_points: RefCell::new(Vec::new()),
            prior_area: std::cell::Cell::new(Rect::ZERO),
            dirty_bit: std::cell::Cell::new(true),
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
        self.draw_corners(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn render_test() {
        let mut buffer = Buffer::empty(Rect::new(0, 0, 20, 20));
        let mut layout = GridLayout::new();
        layout.set_columns(vec![GridDimension {
            min: 0,
            weight: 3
        }, GridDimension {
            min: 2,
            weight: 1
        }]);
        layout.set_rows(vec![GridDimension {
            min: 0,
            weight: 1
        }; 4]);
        layout.render(*buffer.area(), &mut buffer);
        dbg!(buffer);
        dbg!(layout.edge_layout_x);
        dbg!(layout.edge_layout_y);
    }
}
