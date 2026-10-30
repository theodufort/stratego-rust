//! The turn engine on top of [`Board`]: exactly one move or attack per
//! turn (Red first), the Spotter spot rule, anti-shuttling, reveal
//! tracking and the two win conditions (flag capture, and opponent
//! unable to move or attack).

use crate::board::{Board, MoveOutcome};
use crate::error::MoveError;
use crate::piece::PieceName;
use crate::pos::Pos;
use crate::team::{Team, TeamColor};
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
                write!(
                    f,
                    "no piece at that position, or it does not belong to the team"
                )
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
                write!(
                    f,
                    "no enemy piece stands on the square in front of the Spotter"
                )
            }
            GameError::SpotFromOldSquare => {
                write!(
                    f,
                    "a Spotter already moved this turn and may only spot from its new square"
                )
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

        let outcome = self
            .board
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
    /// waiting for this team â€” ordinary turns always end by moving or
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
            && Self::face(from, team).is_some_and(|front| {
                self.board
                    .piece_of(front.x, front.y, &Self::opponent(*team))
                    .is_some()
            })
    }
}

#[cfg(test)]
mod tests;
