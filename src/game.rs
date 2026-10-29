//! The turn engine on top of [`Board`]: exactly one move or attack per
//! turn (Red first), the Spotter spot rule, anti-shuttling, reveal
//! tracking and the two win conditions (flag capture, and opponent
//! unable to move or attack).

use crate::board::{Board, MoveOutcome};
use crate::error::MoveError;
use crate::piece::PieceName;
use crate::pos::Pos;
use crate::team::{TeamColor, Team};
use std::collections::HashSet;
use std::fmt;

/// Why a game ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndGame {
    /// The enemy Flag was captured by `team`.
    FlagCaptured(Team),
    /// The opponent of `team` has no legal move or attack left, so the
    /// team that just acted wins.
    Stalemate(Team),
}

/// Error for an illegal or out-of-order game action.
#[derive(Debug, Clone, PartialEq)]
pub enum GameError {
    /// The game is already over.
    NotFinished,
    /// It is not this team's turn.
    NotYourTurn,
    /// No piece at that position, or it does not belong to the team.
    NotYourPiece,
    /// The move was illegal (see [`MoveError`]).
    Move(MoveError),
    /// The two-shuttle limit was reached: the same two squares may not
    /// be moved on three times in three consecutive turns.
    ShuttleBlocked,
    /// The piece at `from` is not a Spotter.
    NotASpotter,
    /// `from` is out of bounds, or no enemy piece stands on the square
    /// in front of the Spotter.
    NotFacingEnemy,
    /// A Spotter has already moved this turn and may only spot from
    /// its new square.
    SpotFromOldSquare,
    /// This team's Spotter already moved this turn; the turn may only be
    /// ended with `spot` or `end_turn`, no further moves or attacks.
    SpotPending,
    /// There is no pending Spotter spot to resolve, so the turn cannot
    /// be passed: only a Spotter team that just moved may `end_turn`.
    NoSpotPending,
}

impl fmt::Display for GameError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GameError::NotFinished => write!(f, "the game is already over"),
            GameError::NotYourTurn => write!(f, "it is not this team's turn"),
            GameError::NotYourPiece => {
                write!(f, "no piece at that position, or it does not belong to the team")
            }
            GameError::Move(e) => write!(f, "illegal move: {:?}", e),
            GameError::ShuttleBlocked => {
                write!(
                    f,
                    "the same two squares may not be moved on three times in three consecutive turns"
                )
            }
            GameError::NotASpotter => write!(f, "the piece at that position is not a Spotter"),
            GameError::NotFacingEnemy => {
                write!(f, "no enemy piece stands on the square in front of the Spotter")
            }
            GameError::SpotFromOldSquare => {
                write!(f, "a Spotter already moved this turn and may only spot from its new square")
            }
            GameError::SpotPending => {
                write!(
                    f,
                    "the Spotter already moved this turn; end the turn with a spot or pass instead"
                )
            }
            GameError::NoSpotPending => {
                write!(
                    f,
                    "no Spotter spot is pending; the turn can only be ended by moving or attacking"
                )
            }
        }
    }
}

impl std::error::Error for GameError {}

/// A two-player Stratego game.
#[derive(Debug, Clone)]
pub struct Game {
    board: Board,
    turn: Option<Team>,
    /// Set as soon as the game has ended.
    pub over: Option<EndGame>,
    /// One `from`/`to` pair per move already made, tagged with the team
    /// that moved it, in order. Spot actions do not enter this history
    /// (they do not move between two squares) but they still consume a
    /// turn, so they break the "three consecutive turns" window of the
    /// shuttle rule.
    history: Vec<(Team, Pos, Pos)>,
    /// Positions whose piece identity was revealed (a Scout after a
    /// multi-square move, a Spotter after a failed spot).
    revealed: HashSet<Pos>,
    /// Some(team, new square) when that team's Spotter just moved onto
    /// a square with an enemy in front: it may still `spot` this turn.
    spot_after_move: Option<(Team, Pos)>,
}

impl Game {
    /// Create a new game on an empty board. Red always moves first, so
    /// the turn starts with `t2`.
    /// `t1` is unused: Red (`t2`) always moves first.
    pub fn new(_t1: Team, t2: Team) -> Self {
        Self {
            board: Board::new(),
            turn: Some(t2),
            over: None,
            history: Vec::new(),
            revealed: HashSet::new(),
            spot_after_move: None,
        }
    }

    /// Randomly place both armies and (re)start Red's turn.
    pub fn initialize(&mut self, team_blue: &Team, team_red: &Team) {
        self.board.initialize(team_blue, team_red);
        self.turn = Some(*team_red);
        self.over = None;
        self.history.clear();
        self.revealed.clear();
        self.spot_after_move = None;
    }

    pub fn board(&self) -> &Board {
        &self.board
    }

    /// Mutate the board directly (arranging test positions, etc.).
    /// This bypasses the turn machine, so use it only to set up a
    /// position, not to play a turn.
    #[cfg(test)]
    pub(crate) fn board_mut(&mut self) -> &mut Board {
        &mut self.board
    }

    /// Whose turn it is; `None` once the game is over.
    pub fn turn(&self) -> Option<Team> {
        self.turn
    }

    /// The single square in front of a piece: the orthogonal neighbour
    /// on the line toward the *enemy* setup side. Blue pieces (rows
    /// 0-2) face `+y`, Red pieces (rows 5-7) face `-y`. This is the
    /// only square a Spotter may spot.
    fn face(from: Pos, team: &Team) -> Option<Pos> {
        let y = if team.color() == TeamColor::Blue {
            from.y.checked_add(1)?
        } else {
            from.y.checked_sub(1)?
        };
        (y < 8).then(|| Pos::new(from.x, y))
    }

    /// True when a Spotter standing on `pos` would have a legal spot
    /// right now: an enemy piece on the square in front of it.
    fn spot_available_at(&self, pos: Pos, team: &Team) -> bool {
        Self::face(pos, team)
            .map(|front| {
                self.board.piece_at(front.x, front.y).is_some()
                    && !self.board.owned_by(front.x, front.y, team)
            })
            .unwrap_or(false)
    }

    /// Whether the identity of the piece at `pos` was revealed (Scout
    /// long move, failed Spotter spot).
    pub fn revealed(&self, pos: Pos) -> bool {
        self.revealed.contains(&pos)
    }

    /// Move (or attack) `team`'s piece from `from` to `to`.
    ///
    /// One successful call consumes the whole turn: a Scout's
    /// multi-square straight-line move and its attack at the destination
    /// are performed by both this single call, which is how Scouts may
    /// "move and attack in the same turn". Every other piece's
    /// one-square move into an enemy piece is likewise the piece's
    /// attack for the turn.
    ///
    /// Exception: a Spotter that moves onto a square with an enemy in
    /// front keeps the turn and may follow the move with [`Game::spot`]
    /// ("Spotters may move, then use their special attack").
    ///
    /// Ends the game when the Flag is captured, or when the opponent of
    /// `team` has no legal move or attack left; otherwise passes the
    /// turn to the opponent.
    pub fn move_piece(
        &mut self,
        from: Pos,
        to: Pos,
        team: &Team,
    ) -> Result<MoveOutcome, GameError> {
        self.check_turn(team)?;
        // Only one piece may move per turn. A Spotter that just moved
        // with an enemy in front keeps the turn for its spot alone, so
        // no further move or attack is legal on this turn.
        if let Some((spot_team, _)) = self.spot_after_move {
            if spot_team == *team {
                return Err(GameError::SpotPending);
            }
        }
        let piece = self
            .board
            .piece_of(from.x, from.y, team)
            .ok_or(GameError::NotYourPiece)?;
        if self.shuttle_blocked(team, from, to) {
            return Err(GameError::ShuttleBlocked);
        }
        // `Board::move_piece` reports only the fight outcome, so the
        // flag must be remembered before the board mutates.
        let flag_captured = self.board.piece_at(to.x, to.y) == Some(PieceName::Flag)
            && !self.board.owned_by(to.x, to.y, team);

        let outcome = self.board
            .move_piece(from.x, from.y, to.x, to.y, team)
            .map_err(GameError::Move)?;

        // A Scout that moved more than one square reveals itself.
        let steps = from.x.abs_diff(to.x) as i32 + from.y.abs_diff(to.y) as i32;
        if piece == PieceName::Scout && steps > 1 {
            self.revealed.insert(to);
        }

        self.history.push((*team, from, to));
        // A Spotter that just moved may still use its special attack
        // this turn, but only if it moved (did not attack) and an
        // enemy stands on the square in front of its new position;
        // otherwise the move consumes the turn as usual.
        if piece == PieceName::Spotter
            && outcome == MoveOutcome::Moved
            && self.spot_available_at(to, team)
        {
            self.spot_after_move = Some((*team, to));
            self.turn = Some(*team);
        } else {
            self.spot_after_move = None;
            self.finish_or_swap(*team, flag_captured);
        }
        Ok(outcome)
    }

    /// A Spotter guesses which piece stands on the square in front of
    /// it (facing the enemy side).
    ///
    /// `guess` is the piece the Spotter claims is there:
    /// * any ranked guess (`Spotter`..`Marshal`) is correct when the
    ///   target has the same rank,
    /// * a `Bomb` guess is the "no rank" guess and matches any
    ///   rankless target (Spy, Bomb or Flag).
    ///
    /// A correct guess captures the target and the Spotter stays where
    /// it stands. A wrong guess ends the turn without capturing
    /// anything and reveals the Spotter.
    pub fn spot(&mut self, from: Pos, guess: PieceName, team: &Team) -> Result<(), GameError> {
        self.check_turn(team)?;
        let piece = self
            .board
            .piece_of(from.x, from.y, team)
            .ok_or(GameError::NotYourPiece)?;
        if piece != PieceName::Spotter {
            return Err(GameError::NotASpotter);
        }
        let front = Self::face(from, team).ok_or(GameError::NotFacingEnemy)?;
        let target = self
            .board
            .piece_at(front.x, front.y)
            .filter(|_| !self.board.owned_by(front.x, front.y, team))
            .ok_or(GameError::NotFacingEnemy)?;
        if let Some((spot_team, spot_pos)) = self.spot_after_move {
            // The Spotter moved this turn, so the spot may only come
            // from its new square.
            if spot_team != *team || from != spot_pos {
                return Err(GameError::SpotFromOldSquare);
            }
        }
        self.spot_after_move = None;
        let guess_is_correct = match (guess.rank(), target.rank()) {
            // Rank guess vs ranked target: ranks must match exactly.
            (Some(g), Some(t)) => g == t,
            // "No rank" (Bomb) guess matches any rankless target.
            (None, None) => true,
            _ => false,
        };

        if guess_is_correct {
            let flag_captured = self.board.capture_without_moving(front.x, front.y);
            self.finish_or_swap(*team, flag_captured);
        } else {
            // Nothing captured, but the Spotter is revealed.
            self.revealed.insert(from);
            self.finish_or_swap(*team, false);
        }
        Ok(())
    }

    /// End a turn after a Spotter moved with an enemy in front, without
    /// spotting.
    ///
    /// The rules make the follow-up spot optional ("Spotters may move,
    /// then use their special attack"), so instead of forcing a spot
    /// after such a move the team may simply end its turn: the Spotter
    /// keeps the square it moved to, is not revealed, and nothing is
    /// captured.
    ///
    /// Returns [GameError::NoSpotPending] when no Spotter spot is
    /// waiting for this team — ordinary turns always end by moving or
    /// attacking, so passing is only meaningful right after a Spotter
    /// move.
    pub fn end_turn(&mut self, team: &Team) -> Result<(), GameError> {
        self.check_turn(team)?;
        let (spot_team, _) = self
            .spot_after_move
            .filter(|(t, _)| t == team)
            .ok_or(GameError::NoSpotPending)?;
        self.spot_after_move = None;
        self.finish_or_swap(spot_team, false);
        Ok(())
    }

    fn check_turn(&self, team: &Team) -> Result<(), GameError> {
        if self.over.is_some() {
            return Err(GameError::NotFinished);
        }
        if self.turn != Some(*team) {
            return Err(GameError::NotYourTurn);
        }
        Ok(())
    }

    fn finish_or_swap(&mut self, team: Team, flag_captured: bool) {
        if flag_captured {
            self.over = Some(EndGame::FlagCaptured(team));
            self.turn = None;
            return;
        }
        let opponent = Self::opponent(team);
        if self.can_act(&opponent) {
            self.turn = Some(opponent);
        } else {
            // The opponent has no move and no attack: the team that
            // just acted wins.
            self.over = Some(EndGame::Stalemate(team));
            self.turn = None;
        }
    }

    fn opponent(team: Team) -> Team {
        Team::new(match team.color() {
            TeamColor::Blue => TeamColor::Red,
            TeamColor::Red => TeamColor::Blue,
        })
    }

    /// True when the team's two most recent moves swapped the same two
    /// squares (in either direction): a third such move within three
    /// consecutive turns of that team would break the shuttle rule.
    fn shuttle_blocked(&self, team: &Team, from: Pos, to: Pos) -> bool {
        let own: Vec<(Pos, Pos)> = self
            .history
            .iter()
            .rev()
            .filter(|(t, _, _)| t == team)
            .map(|(_, a, b)| (*a, *b))
            .take(2)
            .collect();
        own.len() == 2
            && own
                .iter()
                .all(|(a, b)| (a == &from && b == &to) || (a == &to && b == &from))
    }

    /// True if any of this team's pieces has at least one legal move or
    /// attack (or a Spotter a legal spot). Drives the stalemate win
    /// condition.
    pub fn can_act(&self, team: &Team) -> bool {
        for y in 0..self.board.dim_y {
            for x in 0..self.board.dim_x {
                let Some(piece) = self.board.piece_of(x, y, team) else {
                    continue;
                };
                if self.can_piece_act(Pos::new(x, y), piece, team) {
                    return true;
                }
            }
        }
        false
    }

    /// True if `team`'s `piece` at `from` has at least one legal move
    /// or attack, or (for a Spotter) a legal spot.
    fn can_piece_act(&self, from: Pos, piece: PieceName, team: &Team) -> bool {
        // Bombs and the Flag cannot move or attack at all.
        if piece == PieceName::Bomb || piece == PieceName::Flag {
            return false;
        }
        // Try each of the four straight lines. A piece can move to the
        // first open square of a line (all the way for a Scout) or
        // attack the first enemy piece of a line.
        // Pieces cannot jump: only the first square of each line
        // matters (open = legal move, enemy = legal attack,
        // own piece or crater = blocked).
        for (dx, dy) in [(1i32, 0i32), (-1, 0), (0, 1), (0, -1)] {
            let cx = from.x as i32 + dx;
            let cy = from.y as i32 + dy;
            if cx < 0 || cy < 0 || cx >= self.board.dim_x as i32 || cy >= self.board.dim_y as i32 {
                continue;
            }
            let tx = cx as usize;
            let ty = cy as usize;
            if self.board.tile_is_open(tx, ty) {
                return true; // open square reachable: legal move
            }
            if self
                .board
                .piece_of(tx, ty, &Self::opponent(*team))
                .is_some()
            {
                return true; // enemy on the adjacent square: legal attack
            }
        }
        // Spotter: a legal spot counts as an action
        piece == PieceName::Spotter
            && Self::face(from, team)
                .is_some_and(|front| {
                    self.board
                        .piece_of(front.x, front.y, &Self::opponent(*team))
                        .is_some()
                })
    }
}

#[cfg(test)]
mod tests {
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

            assert_eq!(
                game.board().piece_at(5, 5),
                None,
                "the {target} is spotted"
            );
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
        assert_eq!(
            game.board().piece_at(4, 4),
            None,
            "the target is captured"
        );
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
        assert_eq!(
            game.turn(),
            Some(RED),
            "the pass hands the turn to red"
        );
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
        assert_eq!(
            game.end_turn(&RED),
            Err(GameError::NoSpotPending)
        );
    }

    /// `end_turn` is only legal while one of the team's Spotters is
    /// waiting to spot; otherwise it is refused.
    #[test]
    fn end_turn_is_rejected_without_a_pending_spot() {
        let mut game = empty_game();
        assert_eq!(
            game.end_turn(&RED),
            Err(GameError::NoSpotPending)
        );
        assert_eq!(
            game.end_turn(&BLUE),
            Err(GameError::NotYourTurn)
        );
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
        game.board_mut().place_test_piece(3, 3, PieceName::Spotter, &BLUE);
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
        game.move_piece(a, Pos::new(6, 5), &RED).expect("first swap");
        // turn 2 (blue): scout sidesteps, interleaving.
        game.move_piece(Pos::new(6, 6), Pos::new(7, 6), &BLUE)
            .expect("blue's move interleaves");
        // red's spy can go back and forth only: (6,5) -> (5,5).
        game.move_piece(Pos::new(6, 5), a, &RED).expect("second swap");
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
        assert_eq!(
            out,
            MoveOutcome::Mutual,
            "equal ranks destroy both pieces"
        );
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
}
