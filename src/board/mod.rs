use ndarray::Array2;

struct Board {
    // Dimensions of the board.
    dim_x: usize,
    dim_y: usize,
    tiles: Array2<i32>,
}

impl Board {
    pub fn new() -> Board {
        let dim_x: usize = 10;
        let dim_y: usize = 8;
        Board {
            dim_x,
            dim_y,
            tiles: Array2::<i32>::zeros((dim_x, dim_y)),
        }
    }
}