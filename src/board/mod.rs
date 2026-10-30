use crate::error::MoveError;
use crate::piece::{resolve_fight, BoardPiece, FightResult, PieceName};
use crate::team::Team;
use ndarray::Array2;
use rand::rng;
use rand::seq::SliceRandom;

#[derive(Clone, Copy, Debug, PartialEq)]
enum TilesType {
    // Craters block movement; a fresh board carries the two default
    // patches in its middle rows (see `Board::new`).
    #[allow(dead_code)]
    Crater,
    Empty,
    Piece,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TilePieceOwnership {
    Team1,
    Team2,
    None,
}
#[derive(Clone)]
pub struct Tile {
    piece: Option<BoardPiece>,
    owned_team: TilePieceOwnership,
    tile_type: TilesType,
}

#[derive(Clone)]
pub struct Board {
    // Dimensions of the board.
    pub dim_x: usize,
    pub dim_y: usize,
    pub tiles: Array2<Tile>,
}

// `Tile` is private, so `Board` gets an explicit `Debug` impl that
// prints the pieces (and their owners) row by row.
impl std::fmt::Debug for Board {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Board")
            .field("dim_x", &self.dim_x)
            .field("dim_y", &self.dim_y)
            .field(
                "tiles",
                &self
                    .tiles
                    .iter()
                    .map(|t| (t.piece.as_ref().map(|p| p.id), t.owned_team, t.tile_type))
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

/// The result of a successful `move_piece` call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MoveOutcome {
    /// The piece moved onto an empty tile.
    Moved,
    /// The attack succeeded: the attacker takes the defender's tile and the
    /// defender is removed from the board.
    Captured,
    /// The attack failed: the attacker is removed from the board and the
    /// defender stays in place.
    Lost,
    /// Ranks matched: both pieces are captured and removed from the board.
    Mutual,
}
impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}
impl Board {
    pub fn new() -> Board {
        let dim_x: usize = 10;
        let dim_y: usize = 8;
        let mut board = Board {
            dim_x,
            dim_y,
            tiles: Array2::from_shape_fn((dim_x, dim_y), |_| Tile {
                piece: None,
                tile_type: TilesType::Empty,
                owned_team: TilePieceOwnership::None,
            }),
        };
        // The two crater patches, one in each half of the middle rows.
        for (x, y) in [
            (2, 3),
            (3, 3),
            (2, 4),
            (3, 4),
            (6, 3),
            (7, 3),
            (6, 4),
            (7, 4),
        ] {
            board.tiles[[x, y]].tile_type = TilesType::Crater;
        }
        board
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

        // Each army fills its own three front rows: Blue in rows 0..3, Red in
        // rows 5..8. The two middle rows (3 and 4) stay unoccupied.
        for (team, rows) in [(team_blue, 0..3), (team_red, 5..8)] {
            let ownership = team.ownership();
            let mut pieces: Vec<BoardPiece> = COMPOSITION
                .iter()
                .flat_map(|(name, count)| std::iter::repeat_n(*name, *count).map(BoardPiece::new))
                .collect();
            pieces.shuffle(&mut rng);

            let mut cells: Vec<(usize, usize)> = (0..self.dim_x)
                .flat_map(|x| rows.clone().map(move |y| (x, y)))
                .collect();
            cells.shuffle(&mut rng);

            // Collect into a Vec to end the immutable borrow on self.tiles
            let empty_cells: Vec<(usize, usize)> = cells
                .into_iter()
                .filter(|(x, y)| self.tiles[[*x, *y]].tile_type == TilesType::Empty)
                .take(pieces.len())
                .collect();

            for ((x, y), piece) in empty_cells.into_iter().zip(pieces) {
                let tile = &mut self.tiles[[x, y]];
                tile.piece = Some(piece);
                tile.tile_type = TilesType::Piece;
                tile.owned_team = ownership;
            }
        }
    }

    /// Label shown for a crater square. Longer than `Â·` so a crater can be
    /// told apart from an empty tile at a glance.
    const CRATER_LABEL: &'static str = "crater";

    fn cell_label(&self, tile: &Tile, show_owners: bool) -> String {
        if matches!(tile.tile_type, TilesType::Crater) {
            Self::CRATER_LABEL.to_string()
        } else {
            match &tile.piece {
                Some(piece) if show_owners => {
                    let team = match tile.owned_team {
                        TilePieceOwnership::Team1 => "B:",
                        TilePieceOwnership::Team2 => "R:",
                        TilePieceOwnership::None => "",
                    };
                    format!("{team}{}", piece.id)
                }
                Some(piece) => piece.id.to_string(),
                None => "Â·".to_string(),
            }
        }
    }

    pub fn print_board(&self) {
        self.print_board_inner(false);
    }

    /// Print piece names prefixed with their team (`B:` or `R:`).
    /// This is useful in local games where one person controls both armies.
    pub fn print_board_with_owners(&self) {
        self.print_board_inner(true);
    }

    fn print_board_inner(&self, show_owners: bool) {
        let col_widths: Vec<usize> = (0..self.dim_x)
            .map(|x| {
                (0..self.dim_y)
                    .map(|y| self.cell_label(&self.tiles[[x, y]], show_owners).len())
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
                let s = self.cell_label(&self.tiles[[x, y]], show_owners);
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
    /// Number of steps between two cells on a single axis.
    fn axis_distance(a: usize, b: usize) -> usize {
        a.abs_diff(b)
    }

    /// A normal piece may move exactly one tile up, down, left or right.
    /// Scouts may move any number of tiles, but always along a single axis
    /// and only across open squares: a Scout may not jump over or move
    /// through an occupied square or a crater.
    fn check_move_position(
        &self,
        piece: PieceName,
        from_x: usize,
        from_y: usize,
        to_x: usize,
        to_y: usize,
    ) -> bool {
        let dx = Self::axis_distance(from_x, to_x);
        let dy = Self::axis_distance(from_y, to_y);

        // Stayed in place: not a move.
        if dx == 0 && dy == 0 {
            return false;
        }
        // Moves only along a single axis (no diagonals),
        // and single steps for everything except Scouts.
        if !(dx == 0 || dy == 0) || (piece != PieceName::Scout && dx + dy != 1) {
            return false;
        }

        // A multi-step Scout move needs every square the line crosses to be
        // open; a piece may never occupy a crater anyway, so the destination
        // check below is the same one.
        if piece == PieceName::Scout && dx + dy > 1 {
            let mut x = from_x;
            let mut y = from_y;
            while x != to_x || y != to_y {
                // The move is along one axis: step only the axis that
                // differs, toward the target square.
                if x != to_x {
                    x = if x < to_x { x + 1 } else { x - 1 };
                } else {
                    y = if y < to_y { y + 1 } else { y - 1 };
                }
                let tile = &self.tiles[[x, y]];
                if x == to_x && y == to_y {
                    // The destination may hold an enemy piece to attack;
                    // craters and own pieces are handled by the caller.
                    return !matches!(tile.tile_type, TilesType::Crater);
                }
                if tile.tile_type != TilesType::Empty {
                    return false;
                }
            }
        }
        true
    }
    /// Moves the team's piece from (from_x, from_y) to (to_x, to_y).
    ///
    /// Validation, in order: the origin and destination are on the board, a
    /// piece stands at the origin, the piece belongs to the moving team, the
    /// piece is not immobilized (Bomb, Flag), the move follows that piece's
    /// movement rules, and the destination is not a crater or a tile owned
    /// by the moving team.
    ///
    /// When the destination holds an enemy piece, the move is a fight,
    /// resolved by the rule `resolve_fight`: the winning piece takes the
    /// tile and the loser is removed (`MoveOutcome::Captured` or `Lost`),
    /// and equal ranks remove both pieces (`MoveOutcome::Mutual`). A move
    /// onto an empty tile returns `MoveOutcome::Moved`.
    pub fn move_piece(
        &mut self,
        from_x: usize,
        from_y: usize,
        to_x: usize,
        to_y: usize,
        team: &Team,
    ) -> Result<MoveOutcome, MoveError> {
        // Both origin and destination must exist on the board.
        if from_x >= self.dim_x || from_y >= self.dim_y || to_x >= self.dim_x || to_y >= self.dim_y
        {
            return Err(MoveError::OutOfBounds);
        }

        // A piece stands at the origin and it belongs to this team.
        let from = &self.tiles[[from_x, from_y]];
        let piece_id = from.piece.as_ref().ok_or(MoveError::NoPieceAtPosition)?.id;
        if from.owned_team != team.ownership() {
            return Err(MoveError::NotYourPiece);
        }

        // Bombs and the Flag cannot move at all.
        if piece_id == PieceName::Bomb || piece_id == PieceName::Flag {
            return Err(MoveError::PieceOutOFMoves);
        }

        // The move follows this piece's movement rules, including the Scout
        // requirement that every square along the way is open.
        if !self.check_move_position(piece_id, from_x, from_y, to_x, to_y) {
            return Err(MoveError::PieceOutOFMoves);
        }

        // The destination is not a crater or a tile of the moving team; an
        // enemy piece is not a block but a fight, resolved below.
        let dest = &self.tiles[[to_x, to_y]];
        match dest.tile_type {
            TilesType::Crater => return Err(MoveError::Blocked),
            TilesType::Piece if dest.owned_team == team.ownership() => {
                return Err(MoveError::Blocked);
            }
            _ => {} // empty tile or an enemy piece to fight
        }

        // Snapshot the outcome of the fight (copies only) so that the shared
        // borrow is over before we mutate the board.
        let defending = matches!(dest.tile_type, TilesType::Piece);
        let fight = if defending {
            Some(resolve_fight(
                piece_id,
                dest.piece.as_ref().expect("a Piece tile holds a piece").id,
            ))
        } else {
            None
        };

        // Move: take the piece off the origin tile and clear that tile.
        let moved_piece = self.take_piece(from_x, from_y);

        // Settle the fight the rules prescribe: the winner lands on the
        // destination, the loser is removed, and tied ranks remove both.
        match fight {
            Some(FightResult::Attacker) => {
                self.place_piece(to_x, to_y, moved_piece, team);
                Ok(MoveOutcome::Captured)
            }
            Some(FightResult::Defender) => Ok(MoveOutcome::Lost),
            Some(FightResult::Mutual) => {
                // Both pieces fall: the origin is already empty, clear the
                // defender's tile as well.
                let tile = &mut self.tiles[[to_x, to_y]];
                tile.piece = None;
                tile.tile_type = TilesType::Empty;
                tile.owned_team = TilePieceOwnership::None;
                Ok(MoveOutcome::Mutual)
            }
            None => {
                self.place_piece(to_x, to_y, moved_piece, team);
                Ok(MoveOutcome::Moved)
            }
        }
    }

    /// Removes the piece standing on a tile, resets the tile to empty, and
    /// returns the piece.
    fn take_piece(&mut self, x: usize, y: usize) -> BoardPiece {
        let tile = &mut self.tiles[[x, y]];
        let piece = tile
            .piece
            .take()
            .expect("the origin tile holds the moving piece");
        tile.tile_type = TilesType::Empty;
        tile.owned_team = TilePieceOwnership::None;
        piece
    }

    /// Places a piece on a tile, marking it occupied and owned by the team.
    fn place_piece(&mut self, x: usize, y: usize, piece: BoardPiece, team: &Team) {
        let tile = &mut self.tiles[[x, y]];
        tile.piece = Some(piece);
        tile.tile_type = TilesType::Piece;
        tile.owned_team = team.ownership();
    }

    /// The piece standing on a tile, if any (e.g. after a successful move).
    pub fn piece_at(&self, x: usize, y: usize) -> Option<PieceName> {
        self.tiles[[x, y]].piece.as_ref().map(|p| p.id)
    }

    /// Whether a tile is owned by the given team (only meaningful on Piece
    /// tiles, but it answers the question directly either way).
    pub fn owned_by(&self, x: usize, y: usize, team: &Team) -> bool {
        self.tiles[[x, y]].owned_team == team.ownership()
    }

    /// Whether the tile is occupied by a team's piece (Piece tiles only).
    pub fn piece_of(&self, x: usize, y: usize, team: &Team) -> Option<PieceName> {
        let tile = &self.tiles[[x, y]];
        if tile.tile_type == TilesType::Piece && tile.owned_team == team.ownership() {
            tile.piece.as_ref().map(|p| p.id)
        } else {
            None
        }
    }

    /// Crate-visible test helper so other modules' tests (e.g. game)
    /// can hand-place a piece instead of relying on initialize's
    /// random armies.
    #[doc(hidden)]
    pub fn place_test_piece(&mut self, x: usize, y: usize, id: PieceName, team: &Team) {
        self.tiles[[x, y]] = Tile {
            piece: Some(BoardPiece::new(id)),
            tile_type: TilesType::Piece,
            owned_team: team.ownership(),
        }
    }

    /// Crate-visible test helper: clear a tile back to an empty square.
    #[doc(hidden)]
    pub fn clear_test_tile(&mut self, x: usize, y: usize) {
        let tile = &mut self.tiles[[x, y]];
        tile.piece = None;
        tile.tile_type = TilesType::Empty;
        tile.owned_team = TilePieceOwnership::None;
    }

    /// Crate-visible test helper: turn a square into a crater.
    #[doc(hidden)]
    pub fn set_test_crater(&mut self, x: usize, y: usize) {
        self.tiles[[x, y]].tile_type = TilesType::Crater;
    }

    /// Whether the tile is empty, holds a crater, or is occupied.
    pub fn tile_is_open(&self, x: usize, y: usize) -> bool {
        self.tiles[[x, y]].tile_type == TilesType::Empty
    }

    /// Removes an enemy piece from the spot of a successful Spotter guess
    /// (which does not move the Spotter onto the captured square).
    /// Returns true when the captured piece was the Flag.
    pub fn capture_without_moving(&mut self, x: usize, y: usize) -> bool {
        let captured = self.take_piece(x, y).id;
        captured == PieceName::Flag
    }
}

#[cfg(test)]
mod tests;
