use sled::transaction::UnabortableTransactionError;

#[derive(Clone)]
pub struct TransactionalTree<'tree, K, V> {
    inner: &'tree sled::transaction::TransactionalTree,
    _phantom: core::marker::PhantomData<(K, V)>,
}

impl<'tree, 'value, K, V> TransactionalTree<'tree, K, V>
where
    K: AsRef<[u8]>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    pub(super) fn new(inner: &'tree sled::transaction::TransactionalTree) -> Self {
        Self { inner, _phantom: std::marker::PhantomData }
    }

    #[fehler::throws(UnabortableTransactionError)]
    pub fn insert(&self, key: K, value: &'value V) -> Option<V> {
        let value: sled::IVec = value.into();
        let key: sled::IVec = key.as_ref().into();
        self.inner.insert(key, value)?.map(Into::into)
    }

    #[fehler::throws(UnabortableTransactionError)]
    pub fn get(&self, key: K) -> Option<V> {
        self.inner.get(key)?.map(Into::into)
    }

    #[fehler::throws(UnabortableTransactionError)]
    pub fn remove(&self, key: K) -> Option<V> {
        let key: sled::IVec = key.as_ref().into();
        self.inner.remove(key)?.map(Into::into)
    }

    #[fehler::throws(UnabortableTransactionError)]
    pub fn apply_batch(&self, batch: &Batch<K, V>) {
        self.inner.apply_batch(&batch.0)?
    }

    pub fn flush(&self) {
        self.inner.flush()
    }

    #[fehler::throws(anyhow::Error)]
    pub fn generate_id(&self) -> u64 {
        self.inner.generate_id()?
    }
}

pub struct Batch<K, V>(pub sled::Batch, core::marker::PhantomData<(K, V)>);

impl<K, V> Default for Batch<K, V> {
    fn default() -> Self {
        Self(Default::default(), core::marker::PhantomData)
    }
}

impl<'value, K, V> Batch<K, V>
where
    K: AsRef<[u8]>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    pub fn new() -> Self {
        Default::default()
    }

    pub fn insert(&mut self, key: K, value: V) -> &mut Self {
        let key: sled::IVec = key.as_ref().into();
        let value: sled::IVec = value.into();

        self.0.insert(key, value);
        self
    }

    pub fn remove(&mut self, key: K) -> &mut Self {
        let key: sled::IVec = key.as_ref().into();

        self.0.remove(key);
        self
    }
}

pub struct CompareAndSwapError<V> {
    pub current: Option<V>,
    pub proposed: Option<V>,
}
