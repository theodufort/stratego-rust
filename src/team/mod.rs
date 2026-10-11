use crate::board::{Board, TilePieceOwnership};
use crate::piece::BoardPiece;

pub enum TeamColor{
    Red,
    Blue
}

pub struct Team{
    color: TeamColor
}

impl Team {
    pub fn new(team_color: TeamColor) -> Team {
        Team { color: team_color }
    }

    pub fn ownership(&self) -> TilePieceOwnership {
        match self.color {
            TeamColor::Blue => TilePieceOwnership::Team1,
            TeamColor::Red  => TilePieceOwnership::Team2,
        }
    }
}