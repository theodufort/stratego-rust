#[derive(Debug, Clone, PartialEq)]
pub enum MoveError {
    NoPieceAtPosition,
    NotYourPiece,
    OutOfBounds,
    Blocked,
}