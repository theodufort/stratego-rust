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
    // (4,5) sits clear of the default craters, so all four
    // neighbouring squares are ordinary land.
    for (dx, dy) in [(3, 5), (5, 5), (4, 4), (4, 6)] {
        let mut b = board_with_piece_at(4, 5, PieceName::Captain, &blue());
        assert_eq!(b.move_piece(4, 5, dx, dy, &blue()), Ok(MoveOutcome::Moved));
        assert_occupied(&b, dx, dy, PieceName::Captain, &blue());
        assert_empty(&b, 4, 5);
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
    // Row 5 and column 4 contain no default craters, so every
    // target in this list sits on reachable open terrain.
    for (dx, dy) in [(9, 5), (0, 5), (4, 7), (4, 0), (7, 5)] {
        let mut b = board_with_piece_at(4, 5, PieceName::Scout, &blue());
        assert_eq!(
            b.move_piece(4, 5, dx, dy, &blue()),
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
fn default_craters_fill_the_two_patches_in_the_middle_rows() {
    let b = Board::new();
    let patched = [
        (2, 3),
        (3, 3),
        (2, 4),
        (3, 4),
        (6, 3),
        (7, 3),
        (6, 4),
        (7, 4),
    ];
    for (x, y) in patched {
        assert_eq!(
            b.tiles[[x, y]].tile_type,
            TilesType::Crater,
            "({}, {}) must be a crater",
            x,
            y
        );
    }
    // The squares between the patches are open, untouched terrain.
    for (x, y) in [(4, 3), (5, 3), (4, 0), (5, 5), (0, 2)] {
        assert_eq!(
            b.tiles[[x, y]].tile_type,
            TilesType::Empty,
            "({}, {}) must stay empty",
            x,
            y
        );
    }
}

#[test]
fn default_craters_do_not_affect_army_placement() {
    let mut b = Board::new();
    b.initialize(&blue(), &red());
    let blue_pieces = b
        .tiles
        .iter()
        .filter(|t| t.owned_team == blue().ownership())
        .count();
    let red_pieces = b
        .tiles
        .iter()
        .filter(|t| t.owned_team == red().ownership())
        .count();
    assert_eq!(blue_pieces, 30, "blue keeps its full army of 30");
    assert_eq!(red_pieces, 30, "red keeps its full army of 30");
    // Craters never hold pieces.
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
        assert!(b.tiles[[x, y]].piece.is_none());
    }
}

#[test]
fn moving_onto_a_default_crater_is_blocked() {
    // Each pair places the Captain one square away from a default crater.
    for ((sx, sy), (dx, dy)) in [((5, 4), (6, 4)), ((4, 3), (3, 3))] {
        let mut b = board_with_piece_at(sx, sy, PieceName::Captain, &blue());
        assert_eq!(
            b.move_piece(sx, sy, dx, dy, &blue()),
            Err(MoveError::Blocked),
            "crater at ({}, {}) should block the move",
            dx,
            dy
        );
        assert_occupied(&b, sx, sy, PieceName::Captain, &blue());
    }
}

#[test]
fn a_scout_slide_is_blocked_by_a_default_crater() {
    // Row 3 is open except for (2,3), (3,3) and (6,3), (7,3), so a
    // Scout at (4,3) cannot reach past either patch on that row.
    let mut b = board_with_piece_at(4, 3, PieceName::Scout, &blue());
    assert_eq!(
        b.move_piece(4, 3, 0, 3, &blue()),
        Err(MoveError::PieceOutOFMoves),
        "the western patch at (2,3)/(3,3) must stop the slide"
    );
    assert_eq!(
        b.move_piece(4, 3, 9, 3, &blue()),
        Err(MoveError::PieceOutOFMoves),
        "the eastern patch at (6,3)/(7,3) must stop the slide"
    );
    // Short slides inside the open stretch still work.
    assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Ok(MoveOutcome::Moved));
    assert_occupied(&b, 5, 3, PieceName::Scout, &blue());
    assert_empty(&b, 4, 3);
}

#[test]
fn moving_onto_own_piece_is_blocked() {
    for (dx, dy) in [(5, 3), (4, 2)] {
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
    let mut b = board_with_piece_at(4, 5, PieceName::Scout, &blue());
    add_piece(&mut b, 9, 5, PieceName::Flag, &red());

    assert_eq!(b.move_piece(4, 5, 9, 5, &blue()), Ok(MoveOutcome::Captured));
    assert_occupied(&b, 9, 5, PieceName::Scout, &blue());
    assert_empty(&b, 4, 5);
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
    assert_eq!(
        b.move_piece(4, 3, 5, 3, &blue()),
        Err(MoveError::PieceOutOFMoves)
    );
    assert_eq!(
        b.move_piece(4, 3, 4, 4, &blue()),
        Err(MoveError::PieceOutOFMoves)
    );

    let mut b = board_with_piece_at(4, 3, PieceName::Flag, &blue());
    assert_eq!(
        b.move_piece(4, 3, 5, 3, &blue()),
        Err(MoveError::PieceOutOFMoves)
    );
    // A flag also cannot use its turn to attack an adjacent enemy.
    add_piece(&mut b, 5, 3, PieceName::Spotter, &red());
    assert_eq!(
        b.move_piece(4, 3, 5, 3, &blue()),
        Err(MoveError::PieceOutOFMoves)
    );
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
        assert_eq!(b.move_piece(4, 3, 5, 3, &blue()), Ok(MoveOutcome::Captured));
        assert_occupied(&b, 5, 3, atk, &blue());
        assert_empty(&b, 4, 3);
    }
}
