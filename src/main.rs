use crate::board::Board;
use crate::game::{EndGame, Game};
use crate::piece::PieceName;
use crate::pos::Pos;
use crate::team::{Team, TeamColor};
use std::io::{self, BufRead, Write};

pub mod board;
mod error;
pub mod game;
pub mod piece;
pub mod pos;
pub mod team;

const BLUE: Team = Team::new(TeamColor::Blue);
const RED: Team = Team::new(TeamColor::Red);

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(2);
    }
}

fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        None | Some("board") => show_board(),
        Some("play") => {
            if args.next().is_some() {
                return Err("usage: cargo run -- play".to_string());
            }
            play_game().map_err(|error| error.to_string())
        }
        Some("help") | Some("--help") | Some("-h") => {
            print_usage();
            Ok(())
        }
        Some(command) => Err(format!(
            "unknown mode '{command}'. Run `cargo run -- help` for available modes."
        )),
    }
}

fn show_board() -> Result<(), String> {
    let mut board = Board::new();
    board.initialize(&BLUE, &RED);
    board.print_board_with_owners();
    Ok(())
}

fn play_game() -> io::Result<()> {
    let mut game = Game::new(BLUE, RED);
    game.initialize(&BLUE, &RED);
    println!("Local two-team game. Red moves first.");
    println!("Piece identities are visible to both players in this hot-seat mode.");
    println!("Enter `help` to see commands, or `quit` to leave the game.");

    let stdin = io::stdin();
    let mut input = stdin.lock();
    let mut line = String::new();

    loop {
        game.board().print_board_with_owners();
        if let Some(result) = game.over {
            match result {
                EndGame::FlagCaptured(winner) => {
                    println!("{:?} wins by capturing the flag!", winner.color());
                }
                EndGame::Stalemate(winner) => {
                    println!(
                        "{:?} wins: the opponent has no legal moves.",
                        winner.color()
                    );
                }
            }
            return Ok(());
        }

        let team = game
            .turn()
            .expect("a running game always has a team to move");
        print!("{:?} to move > ", team.color());
        io::stdout().flush()?;
        line.clear();
        if input.read_line(&mut line)? == 0 {
            println!();
            return Ok(());
        }

        let command = line.trim();
        if command.is_empty() {
            continue;
        }
        if command.eq_ignore_ascii_case("quit") {
            return Ok(());
        }
        if command.eq_ignore_ascii_case("help") {
            print_play_help();
            continue;
        }

        match play_command(command, &mut game, &team) {
            Ok(()) => {}
            Err(error) => println!("{error}"),
        }
    }
}

fn play_command(command: &str, game: &mut Game, team: &Team) -> Result<(), String> {
    let parts: Vec<_> = command.split_whitespace().collect();
    match parts
        .first()
        .map(|word| word.to_ascii_lowercase())
        .as_deref()
    {
        Some("move") if parts.len() == 5 => {
            let from = parse_pos(parts[1], parts[2], game.board())?;
            let to = parse_pos(parts[3], parts[4], game.board())?;
            let outcome = game
                .move_piece(from, to, team)
                .map_err(|error| error.to_string())?;
            println!("Move result: {outcome:?}");
            Ok(())
        }
        Some("spot") if parts.len() == 4 => {
            let from = parse_pos(parts[1], parts[2], game.board())?;
            let guess = parse_piece(parts[3])?;
            game.spot(from, guess, team)
                .map_err(|error| error.to_string())?;
            println!("Spot resolved.");
            Ok(())
        }
        Some("pass") if parts.len() == 1 => {
            game.end_turn(team).map_err(|error| error.to_string())?;
            println!("Turn passed.");
            Ok(())
        }
        Some("move" | "spot" | "pass") => {
            Err("Invalid command format. Enter `help` for usage.".into())
        }
        _ => Err("Unknown command. Enter `help` for available commands.".into()),
    }
}

fn parse_pos(x: &str, y: &str, board: &Board) -> Result<Pos, String> {
    let x = x
        .parse::<usize>()
        .map_err(|_| format!("invalid x coordinate '{x}'; coordinates must be numbers"))?;
    let y = y
        .parse::<usize>()
        .map_err(|_| format!("invalid y coordinate '{y}'; coordinates must be numbers"))?;
    if x >= board.dim_x || y >= board.dim_y {
        return Err(format!(
            "coordinate ({x}, {y}) is outside the board; x must be 0..{} and y 0..{}",
            board.dim_x - 1,
            board.dim_y - 1
        ));
    }
    Ok(Pos::new(x, y))
}

fn parse_piece(name: &str) -> Result<PieceName, String> {
    match name.to_ascii_lowercase().as_str() {
        "spotter" => Ok(PieceName::Spotter),
        "scout" => Ok(PieceName::Scout),
        "miner" => Ok(PieceName::Miner),
        "sergeant" => Ok(PieceName::Sergeant),
        "lieutenant" => Ok(PieceName::Lieutenant),
        "captain" => Ok(PieceName::Captain),
        "major" => Ok(PieceName::Major),
        "colonel" => Ok(PieceName::Colonel),
        "general" => Ok(PieceName::General),
        "marshal" => Ok(PieceName::Marshal),
        "spy" => Ok(PieceName::Spy),
        "bomb" => Ok(PieceName::Bomb),
        "flag" => Ok(PieceName::Flag),
        _ => Err(format!("unknown piece '{name}'")),
    }
}

fn print_usage() {
    println!("Stratego modes:");
    println!("  cargo run              Show a randomly arranged board (same as `board`)");
    println!("  cargo run -- board     Show a randomly arranged board with team labels");
    println!("  cargo run -- play      Start a local game controlling both teams");
    println!("  cargo run -- help      Show this help");
}

fn print_play_help() {
    println!("Commands (coordinates are x y, with x=0..9 and y=0..7):");
    println!("  move x y x y       Move a piece or attack the destination");
    println!("  spot x y PieceName Make a Spotter guess (for example: spot 4 3 Marshal)");
    println!("  pass               Pass after a Spotter move with an available spot");
    println!("  help               Show command help");
    println!("  quit               Leave the game");
}
