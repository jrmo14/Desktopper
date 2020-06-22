#[macro_use]
extern crate log;
extern crate pretty_env_logger;

use std::fs::File;
use std::io::BufReader;
use std::ops::DerefMut;
use std::process;

use warp::Filter;

use crate::data_model::DataStore;

const SAVE_FILE_PATH: &str = "todo.json";

#[tokio::main]
async fn main() {
    pretty_env_logger::init();

    info!("My pid is {}", process::id());

    let data_store = DataStore::new();
    match File::open(SAVE_FILE_PATH) {
        Ok(file) => {
            info!("Reading from file");
            match serde_json::from_reader(BufReader::new(file)) {
                Ok(todo) => {
                    info!("Loaded todo from {}", SAVE_FILE_PATH);
                    *data_store.todo_list.write().deref_mut() = todo;
                }
                Err(_) => error!("Unable to load from storage file, invalid data"),
            }
        }
        Err(err) => error!("{}", err),
    }
    let task_routes = filters::task_master(data_store);
    let todo_routes = task_routes.with(warp::log("todo"));
    warp::serve(todo_routes).run(([0, 0, 0, 0], 3030)).await;
}

mod data_model {
    use std::sync::Arc;

    use parking_lot::RwLock;

    use wall_disp::tasks::ToDo;

    #[derive(Clone)]
    pub struct DataStore {
        pub todo_list: Arc<RwLock<ToDo>>,
    }

    impl DataStore {
        pub fn new() -> Self {
            DataStore {
                todo_list: Arc::new(RwLock::new(ToDo::new())),
            }
        }
    }
}

mod filters {
    use std::collections::HashMap;
    use std::str::FromStr;

    use chrono::{DateTime, Local, TimeZone};
    use serde::Deserialize;
    use uuid::Uuid;
    use warp::Filter;

    use crate::{handlers, DataStore};
    use wall_disp::tasks::{Priority, Task};

    pub fn task_master(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::path("todo").and(
            get_task(storage.clone())
                .or(add_task(storage.clone()))
                .or(remove_task(storage.clone()))
                .or(estimate_time(storage.clone()))
                .or(complete(storage.clone()))
                .or(completion_status(storage.clone()))
                .or(search(storage)),
        )
    }

    pub fn get_task(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("get"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::get_task)
    }

    pub fn add_task(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::post()
            .and(warp::path("add"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(json_body())
            .and(with_store(storage))
            .and_then(handlers::add_task)
    }

    pub fn remove_task(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("delete"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::remove_task)
    }

    pub fn estimate_time(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("time"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::estimate_time)
    }

    pub fn complete(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("complete"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::complete)
    }

    pub fn completion_status(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("status"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(with_store(storage))
            .and_then(handlers::completion_status)
    }

    pub fn search(
        storage: DataStore,
    ) -> impl Filter<Extract = impl warp::Reply, Error = warp::Rejection> + Clone {
        warp::get()
            .and(warp::path("search"))
            .and(warp::path::end())
            .and(option_extractor::<Uuid>("uuid"))
            .and(option_extractor::<String>("name"))
            .and(option_extractor::<String>("desc"))
            .and(option_extractor::<DateTime<Local>>("due_date_start"))
            .and(option_extractor::<DateTime<Local>>("due_date_end"))
            .and(option_extractor::<i32>("est_time_low"))
            .and(option_extractor::<i32>("est_time_high"))
            .and(option_extractor::<bool>("complete"))
            .and(option_extractor::<Priority>("priority_low"))
            .and(option_extractor::<Priority>("priority_high"))
            .and(with_store(storage))
            .and_then(handlers::search)
    }

    // IIRC the reason this was needed was b/c Option<Uuid> didn't deserialize
    fn option_extractor<T: FromStr>(
        key: &str,
    ) -> impl Filter<Extract = (Option<T>,), Error = warp::Rejection> + Clone + '_ {
        warp::query::<HashMap<String, String>>().map(
            move |input: HashMap<String, String>| -> Option<T> {
                // Gonna make this hardcoded for now to decode a Option<Uuid> but it should be a good framework for the future
                // Might be able to get serde to handle this though
                match input.get(key) {
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
        // Want those to deserialize during loading from start, so can't mark to not afaik
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
    use std::fs::OpenOptions;

    use uuid::Uuid;
    use warp::{http, Rejection};

    use crate::data_model::DataStore;
    use crate::SAVE_FILE_PATH;
    use chrono::{DateTime, Local};
    use std::ops::Deref;
    use wall_disp::tasks::{CompletionStatus, EstTime, FlattenTasks, Priority, Task};

    pub async fn add_task(
        uuid: Option<Uuid>,
        task: Task,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        info!("Adding task with parent {:?}", uuid);
        let ret_val = match store.todo_list.write().add_task(uuid, task) {
            Ok(()) => Ok(warp::reply::with_status(
                "Added task to todo list",
                http::StatusCode::CREATED,
            )),
            Err(_) => {
                error!("Couldn't find parent id");
                Err(warp::reject::not_found())
            }
        };
        update_file(store);
        ret_val
    }

    pub async fn get_task(
        uuid: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        match uuid {
            Some(uuid) => match store.todo_list.read().get_task(uuid) {
                Some(task) => Ok(warp::reply::json(&task)),
                None => Err(warp::reject::not_found()),
            },
            None => {
                let read_store = store.todo_list.read();
                let tasks = read_store.get_all_tasks();
                Ok(warp::reply::json(&tasks))
            }
        }
    }

    pub async fn remove_task(
        id: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        let ret_val = match id {
            Some(id) => match store.todo_list.write().remove_task(id) {
                Ok(task) => Ok(warp::reply::json(&task)),
                Err(_) => Err(warp::reject::not_found()),
            },
            None => Err(warp::reject::reject()),
        };
        update_file(store);
        ret_val
    }

    pub async fn estimate_time(
        id: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, warp::Rejection> {
        match id {
            Some(id) => match store.todo_list.read().get_task(id) {
                Some(task) => Ok(warp::reply::json(&task.est_time())),
                None => Err(warp::reject()),
            },
            None => Ok(warp::reply::json(&store.todo_list.read().est_time())),
        }
    }

    pub async fn complete(
        id: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, Rejection> {
        match id {
            Some(id) => match store.todo_list.read().get_task(id) {
                Some(task) => Ok(warp::reply::json(&task.complete())),
                None => Err(warp::reject()),
            },
            None => Ok(warp::reply::json(&store.todo_list.read().complete())),
        }
    }

    pub async fn completion_status(
        id: Option<Uuid>,
        store: DataStore,
    ) -> Result<impl warp::Reply, Rejection> {
        match id {
            Some(id) => match store.todo_list.read().get_task(id) {
                Some(task) => Ok(warp::reply::json(&task.completion_status())),
                None => Err(warp::reject()),
            },
            None => Ok(warp::reply::json(
                &store.todo_list.read().completion_status(),
            )),
        }
    }

    pub async fn search(
        id: Option<Uuid>,
        name: Option<String>,
        desc: Option<String>,
        due_date_start: Option<DateTime<Local>>,
        due_date_end: Option<DateTime<Local>>,
        est_time_low: Option<i32>,
        est_time_high: Option<i32>,
        complete: Option<bool>,
        priority_low: Option<Priority>,
        priority_high: Option<Priority>,
        storage: DataStore,
    ) -> Result<impl warp::Reply, Rejection> {
        let todo_list = storage.todo_list.read();
        // Grab the group that we want
        let flat_list = match id {
            Some(id) => match todo_list.get_task(id) {
                Some(task) => task.flatten_tasks().into_iter(),
                None => return Err(warp::reject::custom(InvalidQuery)),
            },
            None => todo_list.flatten_tasks().into_iter(),
        };

        // ALL THE FILTERS
        let search_results: Vec<Task> = flat_list
            .filter(|task| match &name {
                Some(name) => task.get_name().contains(name),
                _ => true,
            })
            .filter(|task| match &desc {
                Some(desc) => task.get_desc().contains(desc),
                _ => true,
            })
            .filter(|task| match due_date_start {
                // Gotta re-wrap to get the behavior from the match being none, while having an ez compare to the get on the task
                Some(due_start) => task.get_due_date() >= Some(due_start),
                _ => true,
            })
            .filter(|task| match due_date_end {
                // Gotta re-wrap to get the behavior from the match being none, while having an ez compare to the get on the task
                Some(due_end) => task.get_due_date() <= Some(due_end),
                _ => true,
            })
            .filter(|task| match est_time_low {
                Some(est_low) => task.est_time() >= est_low,
                _ => true,
            })
            .filter(|task| match est_time_high {
                Some(est_high) => task.est_time() <= est_high,
                _ => true,
            })
            .filter(|task| match complete {
                Some(complete) => {
                    if complete {
                        task.complete()
                    } else {
                        true
                    }
                }
                _ => true,
            })
            .filter(|task| match priority_low {
                // Gotta re-wrap to get the behavior from the match being none, while having an ez compare to the get on the task
                Some(priority) => task.get_priority() >= Some(priority),
                _ => true,
            })
            .filter(|task| match priority_high {
                // Gotta re-wrap to get the behavior from the match being none, while having an ez compare to the get on the task
                Some(priority) => task.get_priority() <= Some(priority),
                _ => true,
            })
            .collect();

        // Say hi
        Ok(warp::reply::json(&search_results))
    }

    pub fn update_file(store: DataStore) {
        match serde_json::to_writer(
            OpenOptions::new()
                .read(true)
                .write(true)
                .create(true)
                .open(SAVE_FILE_PATH)
                .unwrap(),
            store.todo_list.read().deref(),
        ) {
            Ok(_) => {}
            Err(e) => error!("{}", e),
        }
    }

    #[derive(Debug)]
    struct InvalidQuery;
    impl warp::reject::Reject for InvalidQuery {}
}
