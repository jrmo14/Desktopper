mod trello_api;

use crate::trello_api::{TrelloApi, ID};

fn main() {
    let api = TrelloApi::new();
    if !api.is_valid() {
        std::process::exit(1);
    }
    println!("\nBOARDS");
    let boards = api.get_boards().unwrap();
    for board in &boards {
        println!("{:?}", board)
    }
    println!("\nLISTS");
    let board_id = boards[10].get_id();
    let lists = api.get_lists(board_id).unwrap();
    for list in &lists {
        println!("{:?}", list);
    }

    println!("\nCARDS");
    let list_id = lists[0].get_id();
    let cards = api.get_cards(list_id);
    for card in &cards {
        println!("{:?}", card);
    }
}
