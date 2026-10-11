#[derive(Debug, Clone,PartialEq, Copy)]
pub enum PieceName{
    Spotter=1,
    Scout=2,
    Miner=3,
    Sergeant=4,
    Lieutenant=5,
    Captain=6,
    Major=7,
    Colonel=8,
    General=9,
    Marshal=10,
    Spy=11,
    Bomb=12,
    Flag=13
}
impl std::fmt::Display for PieceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}
#[derive(Debug, Clone)]
pub struct BoardPiece{
    pub id: PieceName
}

impl BoardPiece{
    pub fn new(piece_name: PieceName)-> BoardPiece{
        BoardPiece{
            id: piece_name
        }
    }
}