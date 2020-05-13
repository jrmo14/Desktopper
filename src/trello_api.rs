use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json;
use std::env;

pub struct TrelloApi {
    key: Option<String>,
    token: Option<String>,
    client: Client,
}

pub trait ID {
    fn get_id(&self) -> &str;
}

#[derive(Serialize, Deserialize, Debug)]
pub struct Board {
    name: String,
    url: String,
    id: String,
    desc: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct List {
    id: String,
    name: String,
    closed: bool,
    idBoard: String,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Card {
    id: String,
    name: String,
    due: Option<String>,
    dueComplete: bool,
    badges: Badge,
    idChecklists: Vec<String>,
}

#[derive(Serialize, Deserialize, Debug)]
#[allow(non_snake_case)]
pub struct Badge {
    due: Option<String>,
    dueComplete: bool,
    checkItems: i32,
    checkItemsChecked: i32,
}

impl ID for Board {
    fn get_id(&self) -> &str {
        self.id.as_str()
    }
}

impl ID for List {
    fn get_id(&self) -> &str {
        self.id.as_str()
    }
}

impl ID for Card {
    fn get_id(&self) -> &str {
        self.id.as_str()
    }
}

impl TrelloApi {
    pub fn new() -> Self {
        let new_key: Option<String>;
        let new_token: Option<String>;
        match env::var("TRELLO_KEY") {
            Ok(val) => {
                new_key = Some(val);
            }
            Err(err) => {
                new_key = None;
                eprintln!(
                    "Error TRELLO_KEY {:?}\nSet environment variable with token",
                    err
                );
            }
        }
        match env::var("TRELLO_TOKEN") {
            Ok(val) => {
                new_token = Some(val);
            }
            Err(err) => {
                new_token = None;
                eprintln!(
                    "Error TRELLO_TOKEN {:?}\nSet environment variable with token",
                    err
                );
            }
        }
        TrelloApi {
            key: new_key,
            token: new_token,
            client: Client::new(),
        }
    }

    pub fn is_valid(&self) -> bool {
        self.key.is_some() && self.token.is_some()
    }

    pub fn get_boards(&self) -> Option<Vec<Board>> {
        let request_uri: String = format!(
            "https://api.trello.com/1/members/me/boards?\
        &key={key}&token={token}",
            key = self.key.as_ref().unwrap(),
            token = &self.token.as_ref().unwrap()
        );
        let resp = self
            .client
            .get(&request_uri)
            .send()
            .unwrap()
            .text()
            .unwrap();
        let boards: Vec<Board> = serde_json::from_str(resp.as_str()).unwrap();
        Some(boards)
    }

    pub fn get_lists(&self, board_id: &str) -> Option<Vec<List>> {
        let request_uri: String = format!(
            "https://api.trello.com/1/boards/{id}/lists?key={key}&token={token}",
            id = board_id,
            key = self.key.as_ref().unwrap(),
            token = self.token.as_ref().unwrap()
        );
        let resp = self
            .client
            .get(&request_uri)
            .send()
            .unwrap()
            .text()
            .unwrap();
        let lists: Vec<List> = serde_json::from_str(resp.as_str()).unwrap();
        Some(lists)
    }

    pub fn get_cards(&self, list_id: &str) -> Vec<Card> {
        let request_uri: String = format!(
            "https://api.trello.com/1/lists/{id}/cards?key={key}&token={token}",
            id = list_id,
            key = self.key.as_ref().unwrap(),
            token = self.token.as_ref().unwrap()
        );
        let resp = self
            .client
            .get(&request_uri)
            .send()
            .unwrap()
            .text()
            .unwrap();
        println!("{}", resp);
        let cards: Vec<Card> = serde_json::from_str(resp.as_str()).unwrap();
        cards
    }
}
