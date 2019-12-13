use std::cell::RefCell;
use tokio::{
    sync::{mpsc, oneshot},
    task,
};

pub type Uid = usize;

pub struct Database<T: Clone + Send + 'static> {
    sender: RefCell<Option<Sender<T>>>,
    join_handle: RefCell<Option<task::JoinHandle<()>>>,
}
impl<T: Clone + Send + 'static> Database<T> {
    pub fn new(buffer: usize) -> Database<T> {
        let (sender, receiver) = mpsc::channel(buffer);
        let (sender, receiver) = (Sender(sender), Receiver(receiver));

        let join_handle = task::spawn(receiver.handle());

        Database {
            sender: RefCell::new(Some(sender)),
            join_handle: RefCell::new(Some(join_handle)),
        }
    }

    pub fn get(&self) -> Sender<T> {
        self.sender.borrow().clone().expect("Database is closed")
    }

    pub async fn close(&self) {
        drop(self.sender.replace(None));

        let join_handle = self.join_handle.replace(None);
        join_handle
            .expect("Database is already closed")
            .await
            .expect("Failed to await database close");
    }
}

pub struct Query<T: Clone + Send + 'static> {
    pub action: QueryAction<T>,
    pub return_channel: oneshot::Sender<QueryResponse<T>>,
}

#[derive(Clone, Debug)]
pub enum QueryAction<T: Clone + Send + 'static> {
    ReadAll,
    Insert(T),
    GetByUid(Uid),
}

#[derive(Clone, Debug)]
pub enum QueryResponse<T: Clone + Send + 'static> {
    ReadAll(Vec<(Uid, T)>),
    Insert,
    GetByUid(Option<T>),
}

#[derive(Clone)]
pub struct Sender<T: Clone + Send + 'static>(mpsc::Sender<Query<T>>);
impl<T: Clone + Send + 'static> Sender<T> {
    pub async fn query(&self) {}
}

pub struct Receiver<T: Clone + Send + 'static>(mpsc::Receiver<Query<T>>);
impl<T: Clone + Send + 'static> Receiver<T> {
    pub async fn handle(mut self) {
        let mut db: Vec<(Uid, T)> = Vec::new();
        let mut uid_counter = 0;

        while let Some(query) = self.0.recv().await {
            let response: QueryResponse<T> = match query.action {
                QueryAction::ReadAll => QueryResponse::ReadAll(db.clone()),
                QueryAction::Insert(entry) => {
                    uid_counter += 1;
                    db.push((uid_counter, entry));
                    QueryResponse::Insert
                }
                QueryAction::GetByUid(uid) => {
                    let entry = db
                        .iter()
                        .find(|(entry_uid, _)| *entry_uid == uid)
                        .map(|(_, entry)| entry.clone());

                    QueryResponse::GetByUid(entry)
                }
            };

            if let Err(_) = query.return_channel.send(response) {
                panic!("Failed to send query response");
            }
        }
    }
}
