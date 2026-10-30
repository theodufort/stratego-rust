use super::*;

// Board layout used by these deterministic tests: Red's half is
// rows 5..=7 (facing the enemy means facing toward row 4), Blue's
// half rows 0..=2 (facing toward row 3). The middle rows 3..=4 are
// open. `Game::new(BLUE, RED)` gives Red the first move.
const BLUE: Team = Team::new(TeamColor::Blue);
const RED: Team = Team::new(TeamColor::Red);

/// A game on an empty board whose turn machine still starts with Red.
fn empty_game() -> Game {
    Game::new(BLUE, RED)
}

/// Red is the first mover on a fresh game.
#[test]
fn red_moves_first() {
    let game = empty_game();
    assert_eq!(game.turn(), Some(RED));
}

/// The turn alternates; an off-turn attempt is rejected.
#[test]
fn turns_alternate_and_off_turn_is_rejected() {
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(5, 5, PieceName::Scout, &RED);
    b.place_test_piece(5, 1, PieceName::Scout, &BLUE);

    assert_eq!(
        game.move_piece(Pos::new(5, 1), Pos::new(5, 2), &BLUE),
        Err(GameError::NotYourTurn)
    );

    game.move_piece(Pos::new(5, 5), Pos::new(5, 6), &RED)
        .expect("red moves");
    assert_eq!(game.turn(), Some(BLUE));

    game.move_piece(Pos::new(5, 1), Pos::new(5, 2), &BLUE)
        .expect("blue moves");
    assert_eq!(game.turn(), Some(RED));
}

/// A Scout may move and attack in a straight line in one turn.
#[test]
fn scout_moves_and_attacks_in_one_turn() {
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(2, 5, PieceName::Scout, &RED);
    b.place_test_piece(3, 5, PieceName::Spotter, &BLUE);
    b.place_test_piece(5, 1, PieceName::Scout, &BLUE);

    let out = game
        .move_piece(Pos::new(2, 5), Pos::new(3, 5), &RED)
        .expect("scout slides to the enemy and attacks");
    assert_eq!(out, crate::board::MoveOutcome::Captured);
    assert_eq!(game.board().piece_at(3, 5), Some(PieceName::Scout));
    assert!(game.turn() == Some(BLUE));
}

/// Moving a Scout more than one square reveals its identity; a
/// single-square move does not.
#[test]
fn scout_reveals_on_long_moves_only() {
    let mut game = empty_game();
    let b = game.board_mut();
    // Column 0 is clear of craters, so the long slide stays open.
    b.place_test_piece(0, 5, PieceName::Scout, &RED);
    b.place_test_piece(9, 5, PieceName::Scout, &BLUE);

    game.move_piece(Pos::new(0, 5), Pos::new(0, 1), &RED)
        .expect("long straight-line move");
    assert!(
        game.revealed(Pos::new(0, 1)),
        "the long move reveals the Scout"
    );

    game.move_piece(Pos::new(9, 5), Pos::new(9, 4), &BLUE)
        .expect("blue single-square move");
    assert!(
        !game.revealed(Pos::new(9, 4)),
        "a one-square move keeps the Scout hidden"
    );
}

/// The Spy is mobile, unlike Bombs and the Flag.
#[test]
fn spy_is_mobile_bombs_and_flag_are_not() {
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(3, 5, PieceName::Bomb, &RED);
    b.place_test_piece(4, 5, PieceName::Spy, &RED);
    b.place_test_piece(5, 5, PieceName::Flag, &RED);
    b.place_test_piece(3, 1, PieceName::Scout, &BLUE);

    let bomb_from = Pos::new(3, 5);
    assert_eq!(
        game.move_piece(bomb_from, Pos::new(3, 6), &RED),
        Err(GameError::Move(MoveError::PieceOutOFMoves))
    );
    let flag_from = Pos::new(5, 5);
    assert_eq!(
        game.move_piece(flag_from, Pos::new(5, 6), &RED),
        Err(GameError::Move(MoveError::PieceOutOFMoves))
    );

    let spy_from = Pos::new(4, 5);
    game.move_piece(spy_from, Pos::new(4, 6), &RED)
        .expect("the Spy may move");
    assert_eq!(game.board().piece_at(4, 6), Some(PieceName::Spy));
}

/// A correct rank guess captures the enemy piece in front, the
/// Spotter stays put, and the turn passes on.
#[test]
fn spotter_correct_rank_guess_captures_and_stays() {
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(4, 6, PieceName::Spy, &RED);
    b.place_test_piece(4, 4, PieceName::Spotter, &BLUE);
    // Blue faces +y, so the target must stand at (4,5).
    b.place_test_piece(4, 5, PieceName::Major, &RED);
    b.place_test_piece(6, 1, PieceName::Scout, &BLUE);

    // Turn order is red, blue, red, blue: the spot lands on blue's
    // second turn.
    game.move_piece(Pos::new(4, 6), Pos::new(3, 6), &RED)
        .expect("red spy sidesteps");
    game.move_piece(Pos::new(6, 1), Pos::new(6, 0), &BLUE)
        .expect("blue scout sidesteps");
    game.move_piece(Pos::new(3, 6), Pos::new(2, 6), &RED)
        .expect("red spy again");
    let s = Pos::new(4, 4);
    game.spot(s, PieceName::Major, &BLUE)
        .expect("spot succeeds");

    assert_eq!(game.board().piece_at(4, 5), None, "major captured");
    assert_eq!(
        game.board().piece_of(4, 4, &BLUE),
        Some(PieceName::Spotter),
        "the Spotter does not move to the captured square"
    );
    assert!(game.over.is_none());
    assert_eq!(game.turn(), Some(RED));
}

/// A "no rank" (Bomb) guess captures any rankless target: a Spy.
/// Blue faces +y, so the target stands on the square in front of the
/// Spotter.
#[test]
fn bomb_guess_captures_rankless_target() {
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(4, 7, PieceName::Spy, &RED); // red's mover
    b.place_test_piece(5, 4, PieceName::Spotter, &BLUE);
    b.place_test_piece(5, 5, PieceName::Spy, &RED); // in front (rankless)
    b.place_test_piece(6, 0, PieceName::Scout, &BLUE);

    // Turn order: red, blue, red, blue (spot). The mover sidesteps
    // so that it never threatens the target.
    game.move_piece(Pos::new(4, 7), Pos::new(3, 7), &RED)
        .expect("red spy sidesteps");
    game.move_piece(Pos::new(6, 0), Pos::new(6, 1), &BLUE)
        .expect("blue's turn before the spot");
    game.move_piece(Pos::new(3, 7), Pos::new(2, 7), &RED)
        .expect("red spy sidesteps again");
    game.spot(Pos::new(5, 4), PieceName::Bomb, &BLUE)
        .expect("bomb guess vs rankless target");

    assert_eq!(game.board().piece_at(5, 5), None, "spy captured");
    assert_eq!(
        game.board().piece_of(5, 4, &BLUE),
        Some(PieceName::Spotter),
        "the Spotter stays put"
    );
    assert_eq!(game.turn(), Some(RED));
}

/// A Spotter may spot Bombs and the enemy Flag (the rules'
/// explicit exception to "only ranked targets"). Blue faces +y, so
/// the target stands on the square in front of the Spotter.
#[test]
fn spotter_can_spot_bombs_and_the_flag() {
    for target in [PieceName::Bomb, PieceName::Flag] {
        let mut game = empty_game();
        let b = game.board_mut();
        // Red's bomb cannot move and the target is a Bomb/Flag, so
        // the scout is the only red piece with any freedom; it must
        // be able to step twice before the spot.
        b.place_test_piece(0, 7, PieceName::Bomb, &RED);
        b.place_test_piece(2, 7, PieceName::Scout, &RED);
        b.place_test_piece(5, 4, PieceName::Spotter, &BLUE);
        b.place_test_piece(5, 5, target, &RED); // in front of the Spotter
        b.place_test_piece(6, 0, PieceName::Scout, &BLUE);

        // Turn order R, B, R, B: each scout sidesteps, then blue
        // spots (a "no rank" Bomb guess matches both targets).
        game.move_piece(Pos::new(2, 7), Pos::new(2, 6), &RED)
            .expect("red scout steps");
        game.move_piece(Pos::new(6, 0), Pos::new(6, 1), &BLUE)
            .expect("blue scout steps");
        game.move_piece(Pos::new(2, 6), Pos::new(2, 5), &RED)
            .expect("red scout again");
        game.spot(Pos::new(5, 4), PieceName::Bomb, &BLUE)
            .expect("bomb guess vs {target}");

        assert_eq!(game.board().piece_at(5, 5), None, "the {target} is spotted");
    }
}

/// A Spotter may move and then spot in the same turn, but the
/// spot must come from the new square; a Spotter whose move lands
/// with no enemy in front just consumes its turn.
#[test]
fn spotter_moves_then_spots_in_the_same_turn() {
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(1, 6, PieceName::Spy, &RED);
    b.place_test_piece(2, 6, PieceName::Spy, &RED);
    b.place_test_piece(6, 0, PieceName::Spy, &BLUE);
    b.place_test_piece(7, 0, PieceName::Spy, &BLUE);
    b.place_test_piece(5, 3, PieceName::Spotter, &BLUE); // faces +y
    b.place_test_piece(4, 4, PieceName::Scout, &RED); // new front square
    b.place_test_piece(8, 2, PieceName::Spotter, &BLUE); // unmoved twin
    b.place_test_piece(8, 3, PieceName::Spy, &RED); // in front of the twin

    game.move_piece(Pos::new(1, 6), Pos::new(0, 6), &RED)
        .expect("red spy steps");
    game.move_piece(Pos::new(5, 3), Pos::new(4, 3), &BLUE)
        .expect("spotter sidestep");
    // The turn stays blue: the spotter may still spot from its new
    // square, but not from its old one.
    assert_eq!(game.turn(), Some(BLUE), "the turn stays with blue");
    assert_eq!(
        game.spot(Pos::new(5, 3), PieceName::Scout, &BLUE),
        Err(GameError::NotYourPiece),
        "the old square no longer holds the Spotter"
    );
    // The unmoved twin cannot spot either: only the Spotter that
    // just moved keeps the turn.
    assert_eq!(
        game.spot(Pos::new(8, 2), PieceName::Scout, &BLUE),
        Err(GameError::SpotFromOldSquare),
        "another Spotter cannot take the moved one's spot"
    );
    game.spot(Pos::new(4, 3), PieceName::Scout, &BLUE)
        .expect("spot after move");
    assert_eq!(game.board().piece_at(4, 4), None, "the target is captured");
    assert_eq!(
        game.board().piece_at(4, 3),
        Some(PieceName::Spotter),
        "the Spotter stays put"
    );

    // A move with no enemy in front of the new square consumes the
    // turn immediately.
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(1, 6, PieceName::Spy, &RED);
    b.place_test_piece(2, 6, PieceName::Spy, &RED);
    b.place_test_piece(6, 0, PieceName::Spy, &BLUE);
    b.place_test_piece(7, 0, PieceName::Spy, &BLUE);
    b.place_test_piece(5, 3, PieceName::Spotter, &BLUE);
    b.place_test_piece(7, 4, PieceName::Scout, &RED); // not in front

    game.move_piece(Pos::new(1, 6), Pos::new(0, 6), &RED)
        .expect("red spy steps");
    game.move_piece(Pos::new(5, 3), Pos::new(4, 3), &BLUE)
        .expect("spotter sidestep");
    assert_eq!(
        game.turn(),
        Some(RED),
        "no enemy in front: the move consumes the turn"
    );
}

/// After a Spotter moves with an enemy in front, the team's turn may
/// only end with a spot or an explicit pass: any second move (by any
/// piece) is rejected, and `end_turn` passes without revealing or
/// capturing.
#[test]
fn spotting_is_optional_and_a_second_move_is_rejected() {
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(1, 6, PieceName::Spy, &RED);
    b.place_test_piece(2, 6, PieceName::Spy, &RED);
    b.place_test_piece(5, 3, PieceName::Spotter, &BLUE); // faces +y
    b.place_test_piece(4, 4, PieceName::Scout, &RED); // new front square
    b.place_test_piece(8, 2, PieceName::Spotter, &BLUE); // unmoved twin
    b.place_test_piece(8, 3, PieceName::Spy, &RED); // in front of the twin

    game.move_piece(Pos::new(1, 6), Pos::new(0, 6), &RED)
        .expect("red spy steps");
    game.move_piece(Pos::new(5, 3), Pos::new(4, 3), &BLUE)
        .expect("spotter sidestep");
    assert_eq!(game.turn(), Some(BLUE), "the turn stays with blue");

    // A second move by any blue piece - even one with its own spot
    // available - must wait for the spot decision.
    assert_eq!(
        game.move_piece(Pos::new(8, 2), Pos::new(9, 2), &BLUE),
        Err(GameError::SpotPending),
        "the twin Spotter cannot move before the moved one spots or passes"
    );
    assert_eq!(
        game.board().piece_at(8, 2),
        Some(PieceName::Spotter),
        "the rejected move does not disturb the board"
    );

    // Passing ends the turn: no spot, no reveal, no capture.
    game.end_turn(&BLUE).expect("pass the optional spot");
    assert_eq!(game.turn(), Some(RED), "the pass hands the turn to red");
    assert_eq!(
        game.board().piece_at(4, 3),
        Some(PieceName::Spotter),
        "the Spotter stays on its new square"
    );
    assert!(
        !game.revealed(Pos::new(4, 3)),
        "passing does not reveal the Spotter"
    );
    assert_eq!(
        game.board().piece_at(4, 4),
        Some(PieceName::Scout),
        "passing captures nothing"
    );

    // Once the pending spot is cleared, `end_turn` is refused again.
    assert_eq!(game.end_turn(&RED), Err(GameError::NoSpotPending));
}

/// `end_turn` is only legal while one of the team's Spotters is
/// waiting to spot; otherwise it is refused.
#[test]
fn end_turn_is_rejected_without_a_pending_spot() {
    let mut game = empty_game();
    assert_eq!(game.end_turn(&RED), Err(GameError::NoSpotPending));
    assert_eq!(game.end_turn(&BLUE), Err(GameError::NotYourTurn));
}

/// A wrong guess captures nothing, reveals the Spotter, and still
/// consumes the turn. Blue faces +y, so the target stands on the
/// square in front of the Spotter.
#[test]
fn spotter_wrong_guess_fails_and_reveals() {
    let mut game = empty_game();
    let b = game.board_mut();
    // All of red's pieces are boxed in or immovable, so no
    // stalemate can end the game before or after the spot.
    b.place_test_piece(5, 6, PieceName::Spy, &RED);
    b.place_test_piece(1, 6, PieceName::Bomb, &RED);
    b.place_test_piece(7, 1, PieceName::Spy, &RED);
    b.place_test_piece(5, 3, PieceName::Spotter, &BLUE);
    b.place_test_piece(5, 4, PieceName::Spy, &RED); // in front (target)
    b.place_test_piece(6, 1, PieceName::Scout, &BLUE);

    // Turn order: red, blue, red, blue (spot).
    game.move_piece(Pos::new(5, 6), Pos::new(4, 6), &RED)
        .expect("red spy steps");
    game.move_piece(Pos::new(6, 1), Pos::new(6, 0), &BLUE)
        .expect("blue's turn before the spot");
    game.move_piece(Pos::new(4, 6), Pos::new(3, 6), &RED)
        .expect("red spy steps again");
    // Guess Scout (rank 2) against a rankless Spy: wrong.
    game.spot(Pos::new(5, 3), PieceName::Scout, &BLUE)
        .expect("spot resolves");

    assert_eq!(
        game.board().piece_at(5, 4),
        Some(PieceName::Spy),
        "nothing is captured on a wrong guess"
    );
    assert!(
        game.revealed(Pos::new(5, 3)),
        "a wrong guess reveals the Spotter"
    );
    assert_eq!(game.turn(), Some(RED), "the spot consumes the turn");
}

/// A spot on a non-Spotter, or with no enemy in front, is rejected
/// without consuming the turn.
#[test]
fn spot_needs_a_spotter_facing_an_enemy_piece() {
    let mut game = empty_game();
    let b = game.board_mut();
    b.place_test_piece(4, 7, PieceName::Spy, &RED); // red's mover
    b.place_test_piece(3, 3, PieceName::Scout, &BLUE);
    let s = Pos::new(3, 3);

    game.move_piece(Pos::new(4, 7), Pos::new(4, 6), &RED)
        .expect("red spy steps"); // turn is now blue's

    // The piece at `s` is not a Spotter; the rejected call must not
    // consume the turn.
    assert_eq!(
        game.spot(s, PieceName::Scout, &BLUE),
        Err(GameError::NotASpotter)
    );
    assert_eq!(game.turn(), Some(BLUE));

    // Now make it a Spotter, but with an empty square in front.
    game.board_mut().clear_test_tile(3, 3);
    game.board_mut()
        .place_test_piece(3, 3, PieceName::Spotter, &BLUE);
    assert_eq!(
        game.spot(s, PieceName::Scout, &BLUE),
        Err(GameError::NotFacingEnemy)
    );
    assert_eq!(game.turn(), Some(BLUE));
}

/// Moving between the same two squares a third time (within three of
/// the team's own consecutive turns) is blocked, even though the
/// opponent's moves interleave in between.
#[test]
fn shuttle_blocked_after_third_swap() {
    let mut game = empty_game();
    let b = game.board_mut();
    // Red: a lone spy that only wants to dance between two squares;
    // Blue: one mobile scout. No flags, so nothing can ever be
    // captured and no stalemate can interfere before the block.
    b.place_test_piece(5, 5, PieceName::Spy, &RED);
    // Row 6 is clear of craters, so the sidestep path is open.
    b.place_test_piece(6, 6, PieceName::Scout, &BLUE);

    let a = Pos::new(5, 5);

    // turn 1 (red): spy (5,5) -> (6,5).
    game.move_piece(a, Pos::new(6, 5), &RED)
        .expect("first swap");
    // turn 2 (blue): scout sidesteps, interleaving.
    game.move_piece(Pos::new(6, 6), Pos::new(7, 6), &BLUE)
        .expect("blue's move interleaves");
    // red's spy can go back and forth only: (6,5) -> (5,5).
    game.move_piece(Pos::new(6, 5), a, &RED)
        .expect("second swap");
    game.move_piece(Pos::new(7, 6), Pos::new(8, 6), &BLUE)
        .expect("blue again");
    // Third dance within three of red's own turns: blocked.
    assert_eq!(
        game.move_piece(a, Pos::new(6, 5), &RED),
        Err(GameError::ShuttleBlocked),
        "the third swap between the same two squares is blocked"
    );
}

/// Capturing the Flag ends the game for the capturing team.
#[test]
fn flag_capture_ends_the_game() {
    let mut game = empty_game();
    let b = game.board_mut();
    // Red's spy shuttles between (4,5) and (4,6) until the flag is
    // revealed at (4,4); blue's scout must be able to move on both
    // of its early turns, so it never steps onto the flag's line.
    b.place_test_piece(4, 5, PieceName::Spy, &RED);
    b.place_test_piece(3, 7, PieceName::Flag, &RED);
    b.place_test_piece(4, 4, PieceName::Flag, &BLUE);
    // Column 5 is clear of craters, so the scout can sidestep freely.
    b.place_test_piece(5, 1, PieceName::Scout, &BLUE);

    game.move_piece(Pos::new(4, 5), Pos::new(4, 6), &RED)
        .expect("spy backs off");
    game.move_piece(Pos::new(5, 1), Pos::new(5, 2), &BLUE)
        .expect("blue scout steps");
    game.move_piece(Pos::new(4, 6), Pos::new(4, 5), &RED)
        .expect("spy returns to the original square");
    assert!(game.over.is_none(), "no game end yet");

    // The flag is at (4,4): one more blue move, then red steps
    // onto it.
    game.move_piece(Pos::new(5, 2), Pos::new(5, 3), &BLUE)
        .expect("blue scout sidesteps");
    let out = game
        .move_piece(Pos::new(4, 5), Pos::new(4, 4), &RED)
        .expect("spy steps onto the flag");
    assert_eq!(out, crate::board::MoveOutcome::Captured);
    assert_eq!(game.over, Some(EndGame::FlagCaptured(RED)));
    assert_eq!(game.turn(), None);
    // The game is over: nothing more may be played.
    assert_eq!(
        game.move_piece(Pos::new(4, 4), Pos::new(4, 5), &RED),
        Err(GameError::NotFinished)
    );
}

/// When a team to move has no legal move, attack or spot left, the
/// opponent wins the game.
#[test]
fn opponent_without_any_action_loses() {
    let mut game = empty_game();
    {
        let b = game.board_mut();
        // Red: flag and bomb (both immovable) plus one scout.
        // Blue: one scout facing the red scout.
        b.place_test_piece(1, 5, PieceName::Flag, &RED);
        b.place_test_piece(1, 6, PieceName::Bomb, &RED);
        b.place_test_piece(4, 7, PieceName::Scout, &RED);
        b.place_test_piece(4, 5, PieceName::Scout, &BLUE);
    }
    // Turn 1 (red): the scout steps up to the blue scout.
    let out = game
        .move_piece(Pos::new(4, 7), Pos::new(4, 6), &RED)
        .expect("red scout steps");
    assert_eq!(out, MoveOutcome::Moved);
    assert!(game.over.is_none(), "no game end after red's step");

    // Turn 2 (blue): the scout attacks the red scout; equal ranks
    // remove both pieces. Red is left with only the flag and the
    // bomb, which can never move or attack.
    let out = game
        .move_piece(Pos::new(4, 5), Pos::new(4, 6), &BLUE)
        .expect("blue scout attacks");
    assert_eq!(out, MoveOutcome::Mutual, "equal ranks destroy both pieces");
    assert_eq!(
        game.board().piece_at(4, 6),
        None,
        "both scouts are destroyed"
    );
    assert_eq!(
        game.over,
        Some(EndGame::Stalemate(BLUE)),
        "blue wins when red has no action left"
    );
    assert_eq!(game.turn(), None);
}
