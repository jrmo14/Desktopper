use chrono::prelude::{DateTime, Utc};
use reqwest::blocking::Client;
use serde::de::{self, Deserialize, Deserializer, Error, MapAccess, SeqAccess, Visitor};
use serde::ser::{Serialize, SerializeStruct, Serializer};
use serde::Deserialize as DeserializeMacro;
use serde::Serialize as SerializeMacro;
use std::env;
use std::fmt;

pub struct TrelloApi {
    key: Option<String>,
    token: Option<String>,
    client: Client,
}

pub trait ID {
    fn get_id(&self) -> &str;
}

#[derive(SerializeMacro, DeserializeMacro, Debug)]
#[allow(non_snake_case)]
pub struct Board {
    name: String,
    url: String,
    id: String,
    desc: String,
}

#[derive(SerializeMacro, DeserializeMacro, Debug)]
#[allow(non_snake_case)]
pub struct List {
    id: String,
    name: String,
    closed: bool,
    idBoard: String,
}

#[derive(SerializeMacro, DeserializeMacro, Debug)]
#[allow(non_snake_case)]
pub struct Card {
    id: String,
    name: String,
    due: Option<DateTime<Utc>>,
    dueComplete: bool,
    badges: Badge,
    pub idChecklists: Vec<String>,
}

#[derive(SerializeMacro, DeserializeMacro, Debug)]
#[allow(non_snake_case)]
pub struct Badge {
    due: Option<DateTime<Utc>>,
    dueComplete: bool,
    checkItems: i32,
    checkItemsChecked: i32,
}

#[derive(SerializeMacro, DeserializeMacro, Debug, PartialEq)]
#[allow(non_snake_case)]
pub struct Checklist {
    id: String,
    name: String,
    checkItems: Vec<CheckItem>,
}

//#[derive(SerializeMacro, DeserializeMacro, Debug)]
#[derive(Debug, PartialEq)]
#[allow(non_snake_case)]
pub struct CheckItem {
    idChecklist: String,
    state: bool, // Will be complete/incomplete in response from api -> custom (de)serialization
    id: String,
    name: String,
}

impl Card {
    pub fn get_checklist_id(&self, idx: usize) -> &str {
        self.idChecklists[idx].as_str()
    }
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

impl ID for Checklist {
    fn get_id(&self) -> &str {
        self.id.as_str()
    }
}

impl ID for CheckItem {
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

        let resp = self.client.get(&request_uri).send().unwrap();
        let code = resp.status();
        let resp = resp.text().unwrap();
        match code {
            reqwest::StatusCode::OK => {
                let boards: Vec<Board> = serde_json::from_str(resp.as_str()).unwrap();
                Some(boards)
            }
            _ => None,
        }
    }

    pub fn get_lists(&self, board_id: &str) -> Option<Vec<List>> {
        let request_uri: String = format!(
            "https://api.trello.com/1/boards/{id}/lists?key={key}&token={token}",
            id = board_id,
            key = self.key.as_ref().unwrap(),
            token = self.token.as_ref().unwrap()
        );

        let resp = self.client.get(&request_uri).send().unwrap();
        let code = resp.status();
        let resp = resp.text().unwrap();
        match code {
            reqwest::StatusCode::OK => {
                let lists: Vec<List> = serde_json::from_str(resp.as_str()).unwrap();
                Some(lists)
            }
            _ => None,
        }
    }

    pub fn get_cards(&self, list_id: &str) -> Option<Vec<Card>> {
        let request_uri: String = format!(
            "https://api.trello.com/1/lists/{id}/cards?key={key}&token={token}",
            id = list_id,
            key = self.key.as_ref().unwrap(),
            token = self.token.as_ref().unwrap()
        );

        let resp = self.client.get(&request_uri).send().unwrap();
        let code = resp.status();
        let resp = resp.text().unwrap();
        match code {
            reqwest::StatusCode::OK => {
                let cards: Vec<Card> = serde_json::from_str(resp.as_str()).unwrap();
                Some(cards)
            }
            _ => None,
        }
    }

    pub fn get_checklists(&self, list_id: &str) -> Option<Vec<Checklist>> {
        let request_uri: String = format!(
            "https://api.trello.com/1/cards/{id}/checklists?key={key}&token={token}",
            id = list_id,
            key = self.key.as_ref().unwrap(),
            token = self.token.as_ref().unwrap()
        );

        let resp = self.client.get(&request_uri).send().unwrap();
        let code = resp.status();
        let resp = resp.text().unwrap();
        match code {
            reqwest::StatusCode::OK => {
                let checklists: Vec<Checklist> = serde_json::from_str(resp.as_str()).unwrap();
                Some(checklists)
            }
            _ => None,
        }
    }
}

impl<'de> Deserialize<'de> for CheckItem {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Debug)]
        enum Field {
            Id,
            Name,
            State,
            IdChecklist,
            UnknownValue,
        };

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                struct FieldVisitor;
                impl<'de> Visitor<'de> for FieldVisitor {
                    type Value = Field;
                    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                        formatter.write_str("`id` or `name` or `state` or `idChecklist`")
                    }

                    fn visit_str<E>(self, value: &str) -> Result<Field, E>
                    where
                        E: de::Error,
                    {
                        match value {
                            "id" => Ok(Field::Id),
                            "name" => Ok(Field::Name),
                            "state" => Ok(Field::State),
                            "idChecklist" => Ok(Field::IdChecklist),
                            _ => Ok(Field::UnknownValue),
                        }
                    }
                }
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct CheckItemVisitor;
        impl<'de> Visitor<'de> for CheckItemVisitor {
            type Value = CheckItem;
            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("struct CheckItem")
            }

            fn visit_seq<V>(self, mut seq: V) -> Result<CheckItem, V::Error>
            where
                V: SeqAccess<'de>,
            {
                let id_check_str = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(0, &self))?;
                let state_str: String = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(1, &self))?;
                let id_str = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(2, &self))?;
                let name_str = seq
                    .next_element()?
                    .ok_or_else(|| de::Error::invalid_length(3, &self))?;
                Ok(CheckItem {
                    idChecklist: id_check_str,
                    id: id_str,
                    name: name_str,
                    state: !state_str.contains("in"),
                })
            }

            fn visit_map<V>(self, mut map: V) -> Result<CheckItem, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut id_chk = None;
                let mut state_str = None;
                let mut id_str = None;
                let mut name_str = None;
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::IdChecklist => {
                            if id_chk.is_some() {
                                return Err(de::Error::duplicate_field("idChecklist"));
                            }
                            id_chk = Some(map.next_value()?);
                        }
                        Field::State => {
                            if state_str.is_some() {
                                return Err(de::Error::duplicate_field("state"));
                            }
                            state_str = Some(map.next_value()?);
                        }
                        Field::Name => {
                            if name_str.is_some() {
                                return Err(de::Error::duplicate_field("name"));
                            }
                            name_str = Some(map.next_value()?);
                        }
                        Field::Id => {
                            if id_str.is_some() {
                                return Err(de::Error::duplicate_field("id"));
                            }
                            id_str = Some(map.next_value()?);
                        }
                        // Drop the unknown fields (kinda a dirty hack but idk)
                        Field::UnknownValue => {
                            let _: serde_json::Value = map.next_value()?;
                        }
                    }
                }
                let id_chk = id_chk.ok_or_else(|| de::Error::missing_field("idChecklist"))?;
                let state_str: String =
                    state_str.ok_or_else(|| de::Error::missing_field("state"))?;
                let id_str = id_str.ok_or_else(|| de::Error::missing_field("id"))?;
                let name_str = name_str.ok_or_else(|| de::Error::missing_field("name"))?;
                Ok(CheckItem {
                    idChecklist: id_chk,
                    id: id_str,
                    name: name_str,
                    state: !state_str.contains("in"),
                })
            }
        }
        const FIELDS: &[&str] = &["id", "name", "state", "idChecklist"];
        deserializer.deserialize_struct("CheckItem", FIELDS, CheckItemVisitor)
    }
}

impl Serialize for CheckItem {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("CheckItem", 4)?;
        state.serialize_field("idChecklist", &self.idChecklist)?;
        state.serialize_field("id", &self.id)?;
        state.serialize_field("name", &self.name)?;
        state.serialize_field(
            "state",
            match &self.state {
                true => "complete",
                false => "incomplete",
            },
        )?;
        state.end()
    }
}

#[cfg(test)]
mod test {
    use super::{Board, CheckItem, Checklist, List, TrelloApi, ID};
    use serde_json;
    #[test]
    fn check_item_complete_deserialization() {
        let reference = CheckItem {
            name: "Jackson".to_string(),
            id: "0000".to_string(),
            idChecklist: "0000".to_string(),
            state: true,
        };
        let data = r#"
        {
            "name": "Jackson",
            "id": "0000",
            "idChecklist": "0000",
            "state": "complete"
        }"#;
        let test: CheckItem = serde_json::from_str(&data).unwrap();
        assert_eq!(test, reference);
    }

    #[test]
    fn check_item_incomplete_deserialization() {
        let reference = CheckItem {
            name: "Jackson".to_string(),
            id: "0000".to_string(),
            idChecklist: "0000".to_string(),
            state: false,
        };
        let data = r#"
        {
            "name": "Jackson",
            "id": "0000",
            "idChecklist": "0000",
            "state": "incomplete"
        }"#;
        let test: CheckItem = serde_json::from_str(&data).unwrap();
        assert_eq!(test, reference);
    }

    #[test]
    fn check_item_unknown_field_deserialization() {
        let reference = CheckItem {
            name: "Jackson".to_string(),
            id: "0000".to_string(),
            idChecklist: "0000".to_string(),
            state: false,
        };
        let data = r#"
        {
            "name": "Jackson",
            "id": "0000",
            "idChecklist": "0000",
            "state": "incomplete",
            "unknown": "null"
        }"#;
        let test: CheckItem = serde_json::from_str(&data).unwrap();
        assert_eq!(test, reference);
    }

    #[test]
    fn check_item_complete_serialization() {
        let data = CheckItem {
            name: "Jackson".to_string(),
            id: "0000".to_string(),
            idChecklist: "0000".to_string(),
            state: true,
        };
        let reference = r#"
        {
            "idChecklist": "0000",
            "id": "0000",
            "name": "Jackson",
            "state": "complete"
        }"#;
        let test = serde_json::to_string(&data).unwrap();
        assert_eq!(
            test,
            reference
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        );
    }

    #[test]
    fn check_item_incomplete_serialization() {
        let data = CheckItem {
            name: "Jackson".to_string(),
            id: "0000".to_string(),
            idChecklist: "0000".to_string(),
            state: false,
        };
        let reference = r#"
        {
            "idChecklist": "0000",
            "id": "0000",
            "name": "Jackson",
            "state": "incomplete"
        }"#;
        let test = serde_json::to_string(&data).unwrap();
        assert_eq!(
            test,
            reference
                .chars()
                .filter(|c| !c.is_whitespace())
                .collect::<String>()
        );
    }

    #[test]
    fn integration_test() {
        let api = TrelloApi::new();
        if !api.is_valid() {
            std::process::exit(1);
        }

        println!("\nBOARDS");
        let boards = api.get_boards().unwrap();
        for board in &boards {
            println!("{:?}", board)
        }
        // Obv, any of these will fail if the previous one does,
        // but I don't care about that safety for this test/demo

        println!("\nLISTS");
        let board_id = boards[10].get_id();
        let lists = api.get_lists(board_id).unwrap();
        for list in &lists {
            println!("{:?}", list);
        }

        println!("\nCARDS");
        let list_id = lists[0].get_id();
        let cards = api.get_cards(list_id).unwrap();
        for card in &cards {
            println!("{:?}", card);
        }

        println!("\nCHECKLISTS");
        let card_id = cards[0].get_id();
        match api.get_checklists(card_id) {
            Some(lists) => {
                for list in &lists {
                    println!("{:?}", list);
                }
            }
            None => println!(
                "Couldn't retreive a check lists from card with id {}",
                card_id
            ),
        }
    }
}
