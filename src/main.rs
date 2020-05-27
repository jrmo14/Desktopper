mod tasks;
mod trello_api;

use warp::{http, Filter};
use parking_lot::RwLock;
use std::sync::Arc;
use crate::tasks::{ToDo, Task};
use uuid::Uuid;

#[derive(Clone)]
struct DataStore {
    todo_list: Arc<RwLock<ToDo>>,
}

impl DataStore {
    fn new() -> Self {
        DataStore {
            todo_list: Arc::new(RwLock::new(ToDo::new()))
        }
    }
}

async fn add_task(uuid: Option<Uuid>, task: Task, store: DataStore) -> Result<impl warp::Reply, warp::Rejection> {
    match store.todo_list.write().add_task(uuid, task) {
        Ok(()) => {
            Ok(warp::reply::with_status("Added task to todo list", http::StatusCode::CREATED))
        }
        Err(_) => {
            Err(warp::reject::not_found())
        }
    }
}

async fn get_task(uuid: Uuid, store: DataStore) -> Result<impl warp::Reply, warp::Rejection> {
    match store.todo_list.read().get_task(uuid) {
        Some(task) => {
            Ok(warp::reply::json(&task))
        }
        None => Err(warp::reject::not_found())
    }
}

async fn get_all_tasks(store: DataStore) -> Result<impl warp::Reply, warp::Rejection> {
    let tasks = store.todo_list.read().get_all_tasks();
    Ok(warp::reply::json(&tasks))
}


#[tokio::main]
async fn main() {
    let data_store = DataStore::new();
}
