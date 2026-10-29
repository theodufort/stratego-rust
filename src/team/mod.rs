use crate::board::TilePieceOwnership;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TeamColor{
    Red,
    Blue
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Team{
    color: TeamColor
}

impl Team {
    /// `Team` is a plain newtype around its colour, which makes it
    /// cheap to construct; use `const` so it can be used in `const`
    /// expressions and tests.
    pub const fn new(team_color: TeamColor) -> Team {
        Team { color: team_color }
    }

    pub fn color(&self) -> TeamColor {
        self.color
    }

    pub fn ownership(&self) -> TilePieceOwnership {
        match self.color {
            TeamColor::Blue => TilePieceOwnership::Team1,
            TeamColor::Red  => TilePieceOwnership::Team2,
        }
    }
}