use crate::data_types::*;

mod iter;
mod misc;
mod tree;

pub use iter::Iter;
pub use misc::*;
pub use tree::Tree;

#[derive(Clone)]
pub struct Database(sled::Db);

impl Database {
    #[fehler::throws(anyhow::Error)]
    pub fn at(database_dir: impl AsRef<std::path::Path>) -> Self {
        Self(sled::open(database_dir)?)
    }

    #[fehler::throws(anyhow::Error)]
    pub fn users(&self) -> Tree<UserId, User> {
        self.0.open_tree("users")?.into()
    }

    #[fehler::throws(anyhow::Error)]
    pub fn connectors(&self) -> Tree<ConnectorId, Connector> {
        self.0.open_tree("connectors")?.into()
    }

    #[fehler::throws(anyhow::Error)]
    pub fn machines(&self) -> Tree<MachineId, Machine> {
        self.0.open_tree("machines")?.into()
    }
}

#[cfg(test)]
static TEST_DB: once_cell::sync::Lazy<Database> =
    once_cell::sync::Lazy::new(|| Database::at("./target/temp/test_db").unwrap());

#[test]
fn test_basic_tree_operations() {
    TEST_DB.0.drop_tree("connectors").unwrap();

    let connectors = TEST_DB.connectors().unwrap();

    let id: ConnectorId = uuid::Uuid::new_v4().into();

    let connector = Connector {
        id,
        address: "test".into(),
        port: 134,
        timestamp: 128301923,

        owner: uuid::Uuid::nil().into(),

        pin: 1234,
    };

    assert!(connectors.insert(id, &connector).unwrap().is_none());

    assert_eq!(connectors.get(id).unwrap().as_ref(), Some(&connector));

    assert_eq!(
        connectors
            .fetch_and_update(id, |value| {
                let mut new_connector = value?;

                new_connector.port = 5555;

                Some(new_connector)
            })
            .unwrap(),
        Some(connector)
    );

    assert_eq!(connectors.get(id).unwrap().unwrap().port, 5555);
}

#[test]
fn test_batch_and_transaction() {
    TEST_DB.0.drop_tree("machines").unwrap();

    let machines = TEST_DB.machines().unwrap();

    let ids: Vec<_> = (0..20).map(|_| uuid::Uuid::new_v4().into()).collect();

    machines
        .transaction::<(), (), _>(|db| {
            for (i, id) in ids.iter().copied().enumerate() {
                db.insert(id, &Machine {
                    id,
                    number: i as u32,
                    name: format!("Machine #{}", i),
                    model: "Dummy".into(),
                    extension: Extension(0),
                    compat_name_overwrite: Some("N/A".into()),
                    connector: uuid::Uuid::nil().into(),
                    timestamp: i as u64,
                })?;
            }
            Ok(())
        })
        .unwrap();

    assert_eq!(machines.len(), 20);

    let mut batch = Batch::new();

    let ids_to_remove = &ids[5..10];

    let mut remaining_ids = HashSet::new();
    remaining_ids.extend(&ids[..5]);
    remaining_ids.extend(&ids[10..]);

    for id in ids_to_remove {
        batch.remove(*id);
    }

    machines.apply_batch(batch).unwrap();

    assert_eq!(machines.len(), 15);

    use std::collections::HashSet;

    assert_eq!(machines.iter().keys().map(|x| x.unwrap()).collect::<HashSet<_>>(), remaining_ids);
}
