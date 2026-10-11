use crate::board::Board;
use crate::team::{Team, TeamColor};

pub mod board;
pub mod piece;
pub mod team;
mod error;

fn main() {
    let mut board = Board::new();
    let team1 = Team::new(TeamColor::Blue);
    let team2 = Team::new(TeamColor::Red);
    board.initialize(&team1,&team2);
    board.print_board();
}
