#[macro_use]
extern crate log;
extern crate pretty_env_logger;

use std::str::FromStr;
use std::string::ParseError;
use std::sync::Arc;

use parking_lot::RwLock;
use uuid::Uuid;
use warp::filters::sse::data;
use warp::{http, Filter, Rejection};

use crate::tasks::{Task, ToDo};

mod tasks;
mod trello_api;

// Wrap contents to make them thread safe
#[derive(Clone)]
pub struct DataStore {
    todo_list: Arc<RwLock<ToDo>>,
}

impl DataStore {
    fn new() -> Self {
        DataStore {
            todo_list: Arc::new(RwLock::new(ToDo::new())),
        }
    }
}

#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    let data_store = DataStore::new();
    // Test data
    // TODO remove
    let subtask = Task::new("SUB_TEST", "SUB_TEST", None, None, 0, None);
    let mut ref_task = Task::new("TEST", "TEST", None, None, 0, None);
    let subtask2 = Task::new("SUB_TEST2", "SUB_TEST2", None, None, 0, None);

    // Test data
    // TODO remove
    match data_store
        .todo_list
        .write()
        .add_task(None, ref_task.clone())
    {
        Ok(()) => {}
        Err(e) => println!("{}", e),
    }
    // Test data
    // TODO remove
    match data_store
        .todo_list
        .write()
        .add_task(Option::from(ref_task.get_uuid()), subtask.clone())
    {
        Ok(()) => {}
        Err(e) => println!("{}", e),
    }

    match data_store
        .todo_list
        .write()
        .add_task(Option::from(subtask.get_uuid()), subtask2)
    {
        Ok(()) => {}
        Err(e) => println!("{}", e),
    }

    let task_routes = filters::task_master(data_store);
    let todo_routes = task_routes.with(warp::log("todo"));

    warp::serve(todo_routes).run(([127, 0, 0, 1], 3030)).await;
}

mod filters {
    use std::collections::HashMap;
    use std::fmt::Debug;
    use std::hash::Hash;
    use std::str::FromStr;

    use chrono::{DateTime, Local, TimeZone};
    use serde::Deserialize;
    use uuid::Uuid;
    use warp::filters::body::json;
    use warp::{get, Filter};

    use crate::tasks::{Priority, Task};
    use crate::{handlers, DataStore};

    pub fn task_master(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("todo").and(
            get_task(storage.clone())
                .or(get_all_tasks(storage.clone()))
                .or(add_task(storage.clone())),
        )
    }

    pub fn get_task(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("get"))
            .and(warp::path::param::<Uuid>())
            .and(warp::path::end())
            .and(with_store(storage))
            .and_then(handlers::get_task)
    }

    pub fn get_all_tasks(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("get"))
            .and(warp::path::end())
            .and(with_store(storage))
            .and_then(handlers::get_all_tasks)
    }

    pub fn add_task(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::post()
            .and(warp::path("add"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>())
            .and(json_body())
            .and(with_store(storage))
            .and_then(handlers::add_task)
    }

    // IIRC the reason this was needed was b/c Option<Uuid> didn't deserialize
    fn option_extractor<T: FromStr>(
    ) -> impl Filter<Extract = (Option<T>,), Error = warp::Rejection> + Clone {
        warp::query::<HashMap<String, String>>().map(
            |input: HashMap<String, String>| -> Option<T> {
                // Gonna make this hardcoded for now to decode a Option<Uuid> but it should be a good framework for the future
                // Might be able to get serde to handle this though
                match input.get("uuid") {
                    Some(x) => match T::from_str(&*x) {
                        Ok(s) => Some(s),
                        _ => None,
                    },
                    None => None,
                }
            },
        )
    }

    fn with_store(
        storage: DataStore,
    ) -> impl Filter<Extract = (DataStore,), Error = std::convert::Infallible> + Clone {
        warp::any().map(move || storage.clone())
    }

    fn json_body() -> impl Filter<Extract = (Task,), Error = warp::Rejection> + Clone {
        // Make sure that we don't try to deserialize the uuid or sub-tasks
        #[derive(Deserialize)]
        struct ApiTask {
            name: String,
            desc: String,
            due_date: Option<String>,
            est_time: i32,
            priority: Option<Priority>,
        }
        warp::body::content_length_limit(1024 * 16).and(warp::body::json::<ApiTask>().map(
            |x: ApiTask| -> Task {
                let new_due_date = match x.due_date {
                    Some(d) => {
                        match chrono::NaiveDateTime::parse_from_str(d.as_str(), "%Y-%m-%d %H:%M:%S")
                        {
                            Ok(time) => Some(Local.from_local_datetime(&time).unwrap()),
                            _ => None,
                        }
                    }
                    _ => None,
                };
                Task::new(
                    x.name.as_str(),
                    x.desc.as_str(),
                    None,
                    new_due_date,
                    x.est_time,
                    x.priority,
                )
            },
        ))
    }
}

mod handlers {
    use uuid::Uuid;
    use warp::http;

    use crate::tasks::Task;
    use crate::DataStore;

    pub async fn add_task(
        uuid: Option<Uuid>,
        task: Task,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        info!("Adding task with parent {:?}", uuid);
        match store.todo_list.write().add_task(uuid, task) {
            Ok(()) => Ok(warp::reply::with_status(
                "Added task to todo list",
                http::StatusCode::CREATED,
            )),
            Err(_) => {
                error!("R'UH ROH");
                Err(warp::reject::not_found())
            }
        }
    }

    pub async fn get_task(
        uuid: Uuid,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        match store.todo_list.read().get_task(uuid) {
            Some(task) => Ok(warp::reply::json(&task)),
            None => Err(warp::reject::not_found()),
        }
    }

    pub async fn get_all_tasks(store: DataStore) -> Result<impl warp::Reply, warp::Rejection> {
        let tasks = store.todo_list.read().get_all_tasks();
        Ok(warp::reply::json(&tasks))
    }
}
