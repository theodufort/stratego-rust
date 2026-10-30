#[derive(Debug, Clone, PartialEq, Copy)]
pub enum PieceName {
    Spotter,
    Scout,
    Miner,
    Sergeant,
    Lieutenant,
    Captain,
    Major,
    Colonel,
    General,
    Marshal,
    Spy,
    Bomb,
    Flag,
}

impl std::fmt::Display for PieceName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
pub struct BoardPiece {
    pub id: PieceName,
}

impl BoardPiece {
    pub fn new(piece_name: PieceName) -> BoardPiece {
        BoardPiece { id: piece_name }
    }
}

impl PieceName {
    /// The rank printed on the piece. Bombs, the Flag, and the Spy have no
    /// rank: the Spy only wins against the Marshal, Bombs are only destroyed
    /// by a Miner, and the Flag is captured by any attacker.
    pub fn rank(&self) -> Option<u8> {
        match self {
            PieceName::Spotter => Some(1),
            PieceName::Scout => Some(2),
            PieceName::Miner => Some(3),
            PieceName::Sergeant => Some(4),
            PieceName::Lieutenant => Some(5),
            PieceName::Captain => Some(6),
            PieceName::Major => Some(7),
            PieceName::Colonel => Some(8),
            PieceName::General => Some(9),
            PieceName::Marshal => Some(10),
            PieceName::Spy => None,
            PieceName::Bomb => None,
            PieceName::Flag => None,
        }
    }

    /// True for pieces without a rank (Spy, Bomb, Flag). They are the only
    /// pieces that cannot move or attack on their turn.
    pub fn rankless(&self) -> bool {
        matches!(self, PieceName::Spy | PieceName::Bomb | PieceName::Flag)
    }
}

/// Who won a fight and what happened to each fighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FightResult {
    /// The attacker takes the defender's square; the defender is removed.
    Attacker,
    /// The defender stays put; the attacker is removed.
    Defender,
    /// Both pieces are captured and removed from the board.
    Mutual,
}

/// Resolves a fight between an attacking and a defending piece for the two
/// armies, applying the specialist-piece rules:
///
/// * A Bomb beats every attacker except a Miner.
/// * A Spy defeats only the Marshal and loses to every other opponent; any
///   non-Spy piece that attacks a Spy captures it.
/// * Otherwise rank decides: higher wins, equal ranks capture both pieces.
pub fn resolve_fight(attacker: PieceName, defender: PieceName) -> FightResult {
    // Bombs cannot be destroyed except by a Miner.
    if defender == PieceName::Bomb {
        return if attacker == PieceName::Miner {
            FightResult::Attacker
        } else {
            FightResult::Defender
        };
    }
    // The Spy's only victory.
    if attacker == PieceName::Spy && defender == PieceName::Marshal {
        return FightResult::Attacker;
    }
    // The Flag is defenceless against any opponent, Spy included.
    if defender == PieceName::Flag {
        return FightResult::Attacker;
    }
    // A Spy attacks no one successfully otherwise.
    if attacker == PieceName::Spy {
        return FightResult::Defender;
    }
    // A Spy loses to every other piece that attacks it.
    if defender == PieceName::Spy {
        return FightResult::Attacker;
    }
    if attacker.rankless() {
        // Bombs (and in principle the Flag) cannot win an offensive fight.
        return FightResult::Defender;
    }
    // Here both ranks are known: the Spy, Flag and rankless-attacker
    // cases (the only rankless fighters) are handled above.
    let a_rank = attacker.rank().unwrap();
    let d_rank = defender.rank().unwrap();
    match a_rank.cmp(&d_rank) {
        std::cmp::Ordering::Greater => FightResult::Attacker,
        std::cmp::Ordering::Equal => FightResult::Mutual,
        std::cmp::Ordering::Less => FightResult::Defender,
    }
}

#[cfg(test)]
mod tests;
