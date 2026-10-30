use super::*;

fn all() -> [PieceName; 13] {
    [
        PieceName::Spotter,
        PieceName::Scout,
        PieceName::Miner,
        PieceName::Sergeant,
        PieceName::Lieutenant,
        PieceName::Captain,
        PieceName::Major,
        PieceName::Colonel,
        PieceName::General,
        PieceName::Marshal,
        PieceName::Spy,
        PieceName::Bomb,
        PieceName::Flag,
    ]
}

#[test]
fn bomb_beats_everyone_but_miner() {
    for attacker in all() {
        let expected = if attacker == PieceName::Miner {
            FightResult::Attacker
        } else {
            FightResult::Defender
        };
        assert_eq!(
            resolve_fight(attacker, PieceName::Bomb),
            expected,
            "{attacker} vs Bomb"
        );
    }
}

#[test]
fn spy_defeats_only_the_marshal() {
    for defender in all() {
        // The Flag is defenceless, so even a Spy captures it.
        let expected = if defender == PieceName::Marshal || defender == PieceName::Flag {
            FightResult::Attacker
        } else {
            FightResult::Defender
        };
        assert_eq!(
            resolve_fight(PieceName::Spy, defender),
            expected,
            "Spy vs {defender}"
        );
    }
}

#[test]
fn every_other_piece_captures_a_spy() {
    for attacker in all() {
        if attacker == PieceName::Spy {
            continue;
        }
        assert_eq!(
            resolve_fight(attacker, PieceName::Spy),
            FightResult::Attacker,
            "{attacker} vs Spy"
        );
    }
}

#[test]
fn flag_is_defenceless_against_everypiece() {
    for attacker in all() {
        assert_eq!(
            resolve_fight(attacker, PieceName::Flag),
            FightResult::Attacker,
            "{attacker} vs Flag"
        );
    }
}

#[test]
fn higher_rank_wins_lower_rank_loses() {
    assert_eq!(
        resolve_fight(PieceName::Marshal, PieceName::Spotter),
        FightResult::Attacker
    );
    assert_eq!(
        resolve_fight(PieceName::Spotter, PieceName::Marshal),
        FightResult::Defender
    );
    assert_eq!(
        resolve_fight(PieceName::Major, PieceName::Captain),
        FightResult::Attacker
    );
}

#[test]
fn equal_ranks_capture_both_pieces() {
    assert_eq!(
        resolve_fight(PieceName::Captain, PieceName::Captain),
        FightResult::Mutual
    );
    assert_eq!(
        resolve_fight(PieceName::Miner, PieceName::Scout),
        FightResult::Attacker
    );
    assert_eq!(
        resolve_fight(PieceName::Colonel, PieceName::Colonel),
        FightResult::Mutual
    );
}
