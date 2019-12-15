use std::cell::RefCell;
use tokio::{
    sync::{mpsc, oneshot},
    task,
};

pub type Uid = u32;

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
    DeleteByUid(Uid),
}

#[derive(Clone, Debug)]
pub enum QueryResponse<T: Clone + Send + 'static> {
    ReadAll(Vec<(Uid, T)>),
    Insert,
    GetByUid(Option<T>),
    DeleteByUid(bool),
}

#[derive(Clone)]
pub struct Sender<T: Clone + Send + 'static>(mpsc::Sender<Query<T>>);
impl<T: Clone + Send + 'static> Sender<T> {
    pub async fn query(&mut self, action: QueryAction<T>) -> QueryResponse<T> {
        let (sender, receiver) = oneshot::channel();

        let query = Query {
            action,
            return_channel: sender,
        };

        if let Err(_) = self.0.send(query).await {
            panic!("failed to send query");
        }

        receiver.await.expect("failed to receive response")
    }

    pub async fn push(&mut self, entry: T) -> () {
        match self.query(QueryAction::Insert(entry)).await {
            QueryResponse::Insert => (),
            _ => panic!("unexpected response"),
        }
    }

    pub async fn get_all_with_uid(&mut self) -> Vec<(Uid, T)> {
        match self.query(QueryAction::ReadAll).await {
            QueryResponse::ReadAll(entries) => entries,
            _ => panic!("unexpected response"),
        }
    }

    pub async fn get_all(&mut self) -> Vec<T> {
        self.get_all_with_uid().await.into_iter().map(|(_, entry)| entry).collect()
    }

    pub async fn delete_uid(&mut self, uid: Uid) -> bool {
        match self.query(QueryAction::DeleteByUid(uid)).await {
            QueryResponse::DeleteByUid(deleted) => deleted,
            _ => panic!("unexpected response"),
        }
    }
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
                },
                QueryAction::DeleteByUid(uid) => {
                    let index = db
                        .iter()
                        .find(|(entry_uid, _)| *entry_uid == uid)
                        .map(|(uid, _)| uid);


                    let deleted = if let Some(&index) = index {
                        db.remove(index as usize);
                        true
                    }else{
                        false
                    };

                    QueryResponse::DeleteByUid(deleted)
                }
            };

            if let Err(_) = query.return_channel.send(response) {
                panic!("Failed to send query response");
            }
        }
    }
}
