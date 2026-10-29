#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MoveError {
    NoPieceAtPosition,
    NotYourPiece,
    OutOfBounds,
    Blocked,
    PieceOutOFMoves
}