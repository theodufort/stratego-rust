use ndarray::Array2;
use rand::seq::SliceRandom;
use rand::rng;
use crate::error::MoveError;
use crate::team::Team;
use crate::piece::{BoardPiece, FightResult, PieceName, resolve_fight};

#[derive(Clone, Copy, Debug, PartialEq)]
enum TilesType{
    // Craters block movement; they are placed by gameplay code and tests.
    #[allow(dead_code)]
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
#[derive(Clone)]
pub struct Tile{
    piece: Option<BoardPiece>,
    owned_team: TilePieceOwnership,
    tile_type: TilesType
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
                    .map(|t| {
                        (
                            t.piece.as_ref().map(|p| p.id),
                            t.owned_team,
                            t.tile_type,
                        )
                    })
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
    /// Number of steps between two cells on a single axis.
    fn axis_distance(a: usize, b: usize) -> usize {
        a.abs_diff(b)
    }

    /// A normal piece may move exactly one tile up, down, left or right.
    /// Scouts may move any number of tiles, but always along a single axis
    /// and only across open squares: a Scout may not jump over or move
    /// through an occupied square or a crater.
    fn check_move_position(&self,
        piece: PieceName,
        from_x: usize,
        from_y: usize,
        to_x: usize,
        to_y: usize) -> bool {
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
        if from_x >= self.dim_x || from_y >= self.dim_y
            || to_x >= self.dim_x || to_y >= self.dim_y
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
                dest.piece
                    .as_ref()
                    .expect("a Piece tile holds a piece")
                    .id,
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
mod tests {
    use super::*;
    use crate::piece::{BoardPiece, PieceName};
    use crate::team::{Team, TeamColor};

    fn blue() -> Team {
        Team::new(TeamColor::Blue)
    }

    fn red() -> Team {
        Team::new(TeamColor::Red)
    }

    /// Creates a board with a single piece of the given team at (x, y).
fn board_with_piece_at(x: usize, y: usize, id: PieceName, team: &Team) -> Board {
        let mut b = Board::new();
        let tile = &mut b.tiles[[x, y]];
        tile.piece = Some(BoardPiece::new(id));
        tile.tile_type = TilesType::Piece;
        tile.owned_team = team.ownership();
        b
    }

    fn add_piece(b: &mut Board, x: usize, y: usize, id: PieceName, team: &Team) {
        let tile = &mut b.tiles[[x, y]];
        tile.piece = Some(BoardPiece::new(id));
        tile.tile_type = TilesType::Piece;
        tile.owned_team = team.ownership();
    }

    fn set_crater(b: &mut Board, x: usize, y: usize) {
        b.tiles[[x, y]].tile_type = TilesType::Crater;
    }

    fn assert_occupied(b: &Board, x: usize, y: usize, id: PieceName, team: &Team) {
        let tile = &b.tiles[[x, y]];
        assert_eq!(tile.piece.as_ref().map(|p| p.id), Some(id));
        assert_eq!(tile.owned_team, team.ownership());
        assert_eq!(tile.tile_type, TilesType::Piece);
    }

    fn assert_empty(b: &Board, x: usize, y: usize) {
        let tile = &b.tiles[[x, y]];
        assert!(tile.piece.is_none());
        assert_eq!(tile.owned_team, TilePieceOwnership::None);
        assert_eq!(tile.tile_type, TilesType::Empty);
    }

    #[test]
    fn normal_piece_moves_one_step_in_each_direction() {
        for (dx, dy) in [(3, 3), (5, 3), (4, 2), (4, 4)] {
            let mut b = board_with_piece_at(4, 3, PieceName::Captain, &blue());
            assert_eq!(b.move_piece(4, 3, dx, dy, &blue()), Ok(MoveOutcome::Moved));
            assert_occupied(&b, dx, dy, PieceName::Captain, &blue());
            assert_empty(&b, 4, 3);
        }
    }

    #[test]
    fn staying_in_place_is_rejected() {
        let mut b = board_with_piece_at(4, 3, PieceName::Captain, &blue());
        assert_eq!(
            b.move_piece(4, 3, 4, 3, &blue()),
            Err(MoveError::PieceOutOFMoves)
        );
    }

    #[test]
    fn diagonal_moves_are_rejected() {
        for (dx, dy) in [(3, 2), (5, 4), (3, 4), (5, 2)] {
            let mut b = board_with_piece_at(4, 3, PieceName::Captain, &blue());
            assert_eq!(
                b.move_piece(4, 3, dx, dy, &blue()),
                Err(MoveError::PieceOutOFMoves),
                "diagonal move to ({}, {}) should be rejected",
                dx,
                dy
            );
        }
    }

    #[test]
    fn normal_piece_cannot_move_more_than_one_step() {
        for (dx, dy) in [(6, 3), (2, 3), (9, 3), (4, 5), (4, 1), (4, 0), (0, 3)] {
            let mut b = board_with_piece_at(4, 3, PieceName::Captain, &blue());
            assert_eq!(
                b.move_piece(4, 3, dx, dy, &blue()),
                Err(MoveError::PieceOutOFMoves),
                "multi-step move to ({}, {}) should be rejected",
                dx,
                dy
            );
        }
    }

    #[test]
    fn scout_moves_multiple_tiles_along_axis() {
        for (dx, dy) in [(9, 3), (0, 3), (4, 7), (4, 0), (7, 3)] {
            let mut b = board_with_piece_at(4, 3, PieceName::Scout, &blue());
            assert_eq!(
                b.move_piece(4, 3, dx, dy, &blue()),
                Ok(MoveOutcome::Moved),
                "scout should reach ({}, {})",
                dx,
                dy
            );
            assert_occupied(&b, dx, dy, PieceName::Scout, &blue());
            assert_empty(&b, 4, 3);
        }
    }

    #[test]
    fn scout_cannot_move_diagonally() {
        for (dx, dy) in [(8, 7), (0, 0), (5, 2)] {
            let mut b = board_with_piece_at(4, 3, PieceName::Scout, &blue());
            assert_eq!(
                b.move_piece(4, 3, dx, dy, &blue()),
                Err(MoveError::PieceOutOFMoves),
                "scout diagonal move to ({}, {}) should be rejected",
                dx,
                dy
            );
        }
    }

    #[test]
    fn moving_an_empty_origin_returns_no_piece() {
        for ((x, y), (tx, ty)) in [((0, 0), (1, 0)), ((4, 3), (4, 4)), ((9, 7), (8, 7))] {
            let mut b = Board::new();
            assert_eq!(
                b.move_piece(x, y, tx, ty, &blue()),
                Err(MoveError::NoPieceAtPosition),
                "origin ({}, {}) is empty",
                x,
                y
            );
        }
    }

    #[test]
    fn moving_an_enemy_piece_returns_not_your_piece() {
        // Enemy piece: the owning team check runs before the movement check.
        let mut b = board_with_piece_at(4, 3, PieceName::Captain, &red());
        assert_eq!(
            b.move_piece(4, 3, 5, 3, &blue()),
            Err(MoveError::NotYourPiece)
        );
        // The owner can still move their own piece: control case.
        assert_eq!(b.move_piece(4, 3, 5, 3, &red()), Ok(MoveOutcome::Moved));
        assert_occupied(&b, 5, 3, PieceName::Captain, &red());
    }

    #[test]
    fn out_of_bounds_positions_rejected() {
        // Destinations outside the board.
        for (dx, dy) in [(10, 3), (4, 8), (99, 1)] {
            let mut b = board_with_piece_at(4, 3, PieceName::Captain, &blue());
            assert_eq!(
                b.move_piece(4, 3, dx, dy, &blue()),
                Err(MoveError::OutOfBounds),
                "destination ({}, {}) is off the board",
                dx,
                dy
            );
        }
        // Origins outside the board.
        for (ox, oy) in [(10, 3), (4, 8)] {
            let mut b = Board::new();
            add_piece(&mut b, 4, 3, PieceName::Captain, &blue());
            assert_eq!(
                b.move_piece(ox, oy, 5, 3, &blue()),
                Err(MoveError::OutOfBounds),
                "origin ({}, {}) is off the board",
                ox,
                oy
            );
        }
    }

    #[test]
    fn moving_onto_a_crater_is_blocked() {
        for (dx, dy) in [(5, 3), (4, 2)] {
            let mut b = board_with_piece_at(4, 3, PieceName::Captain, &blue());
            set_crater(&mut b, dx, dy);
            assert_eq!(
                b.move_piece(4, 3, dx, dy, &blue()),
                Err(MoveError::Blocked),
                "crater at ({}, {}) should block the move",
                dx,
                dy
            );
            assert_occupied(&b, 4, 3, PieceName::Captain, &blue());
        }
    }

    #[test]
    fn moving_onto_own_piece_is_blocked() {
        for (dx, dy) in [(5, 3), (4, 4)] {
            let mut b = board_with_piece_at(4, 3, PieceName::Captain, &blue());
            add_piece(&mut b, dx, dy, PieceName::Scout, &blue());
            assert_eq!(
                b.move_piece(4, 3, dx, dy, &blue()),
                Err(MoveError::Blocked),
                "own piece at ({}, {}) should block the move",
                dx,
                dy
            );
            assert_occupied(&b, 4, 3, PieceName::Captain, &blue());
            assert_occupied(&b, dx, dy, PieceName::Scout, &blue());
        }
    }

    #[test]
    fn moving_onto_enemy_piece_captures_it() {
        let mut b = board_with_piece_at(4, 3, PieceName::Colonel, &blue());
        add_piece(&mut b, 5, 3, PieceName::Captain, &red());

        assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Ok(MoveOutcome::Captured));
        assert_occupied(&b, 5, 3, PieceName::Colonel, &blue());
        assert_empty(&b, 4, 3);
    }

    #[test]
    fn scout_captures_an_enemy_far_away() {
        let mut b = board_with_piece_at(4, 3, PieceName::Scout, &blue());
        add_piece(&mut b, 9, 3, PieceName::Flag, &red());

        assert_eq!(b.move_piece(4, 3, 9, 3, &blue()), Ok(MoveOutcome::Captured));
        assert_occupied(&b, 9, 3, PieceName::Scout, &blue());
        assert_empty(&b, 4, 3);
    }

    #[test]
    fn consecutive_moves_of_a_same_piece() {
        let mut b = board_with_piece_at(4, 3, PieceName::Captain, &blue());
        assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Ok(MoveOutcome::Moved));
        assert_eq!(b.move_piece(5, 3, 5, 4, &blue()), Ok(MoveOutcome::Moved));
        assert_occupied(&b, 5, 4, PieceName::Captain, &blue());
        assert_empty(&b, 4, 3);
        assert_empty(&b, 5, 3);
    }

    #[test]
    fn stronger_attacker_captures_and_keeps_the_position() {
        // For every pair the attacker rating is strictly higher.
        for (atk, def) in [
            (PieceName::Colonel, PieceName::Captain),
            (PieceName::General, PieceName::Colonel),
            (PieceName::Marshal, PieceName::General),
            (PieceName::Miner, PieceName::Bomb),
        ] {
            let mut b = board_with_piece_at(4, 3, atk, &blue());
            add_piece(&mut b, 5, 3, def, &red());

            assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Ok(MoveOutcome::Captured));
            assert_occupied(&b, 5, 3, atk, &blue());
            assert_empty(&b, 4, 3);
        }
    }

    #[test]
    fn weaker_or_equal_attacker_loses_the_piece() {
        // Weaker and the spy-vs-bomb case all lose the fight.
        for (atk, def) in [
            (PieceName::Captain, PieceName::Colonel),
            (PieceName::Spy, PieceName::Bomb),
            (PieceName::Spotter, PieceName::Marshal),
        ] {
            let mut b = board_with_piece_at(4, 3, atk, &blue());
            add_piece(&mut b, 5, 3, def, &red());

            assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Ok(MoveOutcome::Lost));
            // The defender survives with its team, the attacker is gone.
            assert_occupied(&b, 5, 3, def, &red());
            assert_empty(&b, 4, 3);
        }
    }

    #[test]
    fn equal_ranks_remove_both_pieces() {
        let mut b = board_with_piece_at(4, 3, PieceName::Colonel, &blue());
        add_piece(&mut b, 5, 3, PieceName::Colonel, &red());

        assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Ok(MoveOutcome::Mutual));
        assert_empty(&b, 4, 3);
        assert_empty(&b, 5, 3);
    }

    #[test]
    fn bombs_and_flags_cannot_move() {
        let mut b = board_with_piece_at(4, 3, PieceName::Bomb, &blue());
        assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Err(MoveError::PieceOutOFMoves));
        assert_eq!(b.move_piece(4, 3, 4, 4, &blue()), Err(MoveError::PieceOutOFMoves));

        let mut b = board_with_piece_at(4, 3, PieceName::Flag, &blue());
        assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Err(MoveError::PieceOutOFMoves));
        // A flag also cannot use its turn to attack an adjacent enemy.
        add_piece(&mut b, 5, 3, PieceName::Spotter, &red());
        assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Err(MoveError::PieceOutOFMoves));
        assert_occupied(&b, 4, 3, PieceName::Flag, &blue());
    }

    #[test]
    fn flag_is_defenceless_against_every_piece() {
        let all = [
            PieceName::Spotter,
            PieceName::Scout,
            PieceName::Miner,
            PieceName::Sergeant,
            PieceName::Lieutenant,
            PieceName::Captain,
            PieceName::Major,
            PieceName::Colonel,
            PieceName::General,
            PieceName::Marshal,
            PieceName::Spy,
        ];
        for atk in all {
            let mut b = board_with_piece_at(4, 3, atk, &blue());
            add_piece(&mut b, 5, 3, PieceName::Flag, &red());
            assert_eq!(
                b.move_piece(4, 3, 5, 3, &blue()),
                Ok(MoveOutcome::Captured)
            );
            assert_occupied(&b, 5, 3, atk, &blue());
            assert_empty(&b, 4, 3);
        }
    }
}
