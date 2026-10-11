use ndarray::Array2;
use rand::seq::SliceRandom;
use rand::rng;
use crate::board::TilesType::Piece;
use crate::error::MoveError;
use crate::team::Team;
use crate::piece::{BoardPiece, PieceName};

#[derive(PartialEq)]
enum TilesType{
    Crater,
    Empty,
    Piece
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePieceOwnership {
    Team1,
    Team2,
    None
}
pub struct Tile{
    piece: Option<BoardPiece>,
    owned_team: TilePieceOwnership,
    tile_type: TilesType
}

pub struct Board {
    // Dimensions of the board.
    pub dim_x: usize,
    pub dim_y: usize,
    pub tiles: Array2<Tile>,
}

impl Board {
    pub fn new() -> Board {
        let dim_x: usize = 10;
        let dim_y: usize = 8;
        Board {
            dim_x,
            dim_y,
            tiles: Array2::from_shape_fn((dim_x, dim_y), |_| Tile {
                piece: None,
                tile_type: TilesType::Empty,
                owned_team: TilePieceOwnership::None
            }),
        }
    }
    pub fn initialize(&mut self, team_blue: &Team, team_red: &Team) {
        const COMPOSITION: &[(PieceName, usize)] = &[
            (PieceName::Spotter, 2),
            (PieceName::Scout, 5),
            (PieceName::Miner, 5),
            (PieceName::Sergeant, 2),
            (PieceName::Lieutenant, 2),
            (PieceName::Captain, 2),
            (PieceName::Major, 1),
            (PieceName::Colonel, 1),
            (PieceName::General, 1),
            (PieceName::Marshal, 1),
            (PieceName::Spy, 1),
            (PieceName::Bomb, 6),
            (PieceName::Flag, 1),
        ];

        let mut rng = rng();

        for (team, ownership) in [(team_blue, TilePieceOwnership::Team1), (team_red, TilePieceOwnership::Team2)] {
            let mut pieces: Vec<BoardPiece> = COMPOSITION
                .iter()
                .flat_map(|(name, count)| std::iter::repeat(*name).take(*count).map(BoardPiece::new))
                .collect();
            pieces.shuffle(&mut rng);

            let mut cells: Vec<(usize, usize)> = (0..self.dim_x)
                .flat_map(|x| (0..self.dim_y).map(move |y| (x, y)))
                .collect();
            cells.shuffle(&mut rng);

            // Collect into a Vec to end the immutable borrow on self.tiles
            let empty_cells: Vec<(usize, usize)> = cells
                .into_iter()
                .filter(|(x, y)| self.tiles[[*x, *y]].tile_type == TilesType::Empty && *y != 3 && *y != 4)
                .take(pieces.len())
                .collect();

            for ((x, y), piece) in empty_cells.into_iter().zip(pieces) {
                let tile = &mut self.tiles[[x, y]];
                tile.piece = Some(piece);
                tile.tile_type = TilesType::Piece;
                tile.owned_team = ownership.clone();
            }
        }
    }

    pub fn print_board(&self) {
        let col_widths: Vec<usize> = (0..self.dim_x)
            .map(|x| {
                (0..self.dim_y)
                    .map(|y| self.tiles[[x, y]].piece.as_ref().map_or(1, |p| p.id.to_string().len()))
                    .max()
                    .unwrap_or(1)
                    .max(4)
            })
            .collect();

        // Helper closure to avoid fighting format strings
        let pad = |s: &str, w: usize| format!("{:>1$}", s, w);

        // Column index header
        print!("     |");
        for (x, &w) in col_widths.iter().enumerate() {
            print!("{} |", pad(&x.to_string(), w));
        }
        println!();

        // Separator
        print!("     +");
        for &w in &col_widths {
            print!("{}+", "-".repeat(w));
        }
        println!();

        // Rows
        for y in 0..self.dim_y {
            print!("{:>4} |", y);
            for (x, &w) in col_widths.iter().enumerate() {
                let cell = &self.tiles[[x, y]];
                let s = match &cell.piece {
                    Some(piece) => piece.id.to_string(),
                    None => "·".to_string(),
                };
                print!("{} |", pad(&s, w));
            }
            println!();
        }
    }
    pub fn search_piece(
        &self,
        pos_x: usize,
        pos_y: usize,
        team: &Team,
        piece: &PieceName,
    ) -> Option<&BoardPiece> {
        let tile = &self.tiles[[pos_x, pos_y]];
        let p = tile.piece.as_ref()?;

        if p.id == *piece && tile.owned_team == team.ownership() {
            Some(p)
        } else {
            None
        }
    }
    pub fn move_piece(
        &mut self,
        from_x: usize,
        from_y: usize,
        to_x: usize,
        to_y: usize,
        team: &Team,
    ) -> Result<(), MoveError> {
        // Validate: piece exists and belongs to this team
        let tile = &self.tiles[[from_x, from_y]];
        let piece = tile.piece.as_ref()
            .ok_or(MoveError::NoPieceAtPosition)?;
        if tile.owned_team != team.ownership() {
            return Err(MoveError::NotYourPiece);
        }

        // Validate: destination is in bounds and empty
        if to_x >= self.dim_x || to_y >= self.dim_y {
            return Err(MoveError::OutOfBounds);
        }
        let dest = &self.tiles[[to_x, to_y]];
        if dest.tile_type != TilesType::Empty && dest.tile_type != TilesType::Piece {
            return Err(MoveError::Blocked); // or capture logic
        }

        // Move: extract piece from origin
        let piece = self.tiles[[from_x, from_y]].piece.take()
            .unwrap(); // safe, we just checked
        self.tiles[[from_x, from_y]].tile_type = TilesType::Empty;
        self.tiles[[from_x, from_y]].owned_team = TilePieceOwnership::Team1; // reset or remove

        // Place piece at destination
        self.tiles[[to_x, to_y]].piece = Some(piece);
        self.tiles[[to_x, to_y]].tile_type = TilesType::Piece;
        self.tiles[[to_x, to_y]].owned_team = team.ownership();

        Ok(())
    }
}