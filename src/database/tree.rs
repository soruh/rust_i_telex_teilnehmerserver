use super::{Batch, CompareAndSwapError, Iter, TransactionalTree};

pub use sled::transaction::{ConflictableTransactionResult, UnabortableTransactionError};

#[derive(Clone)]
pub struct Tree<K, V> {
    inner: sled::Tree,
    _phantom: core::marker::PhantomData<(K, V)>,
}

impl<'value, K: std::fmt::Debug, V: std::fmt::Debug> std::fmt::Debug for Tree<K, V>
where
    K: AsRef<[u8]> + From<sled::IVec>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut struct_debugger = f.debug_struct(core::any::type_name::<Self>());

        for (key, value) in self.iter().map(|x| x.unwrap()) {
            struct_debugger.field(&format!("{:?}", key), &value);
        }

        struct_debugger.finish()
    }
}

impl<'value, K, V> From<sled::Tree> for Tree<K, V>
where
    K: AsRef<[u8]>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    fn from(tree: sled::Tree) -> Self {
        Self { inner: tree, _phantom: core::marker::PhantomData }
    }
}

impl<'value, K, V> Tree<K, V>
where
    K: AsRef<[u8]>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    #[fehler::throws(anyhow::Error)]
    pub fn insert(&self, key: K, value: &'value V) -> Option<V> {
        let value: sled::IVec = value.into();
        self.inner.insert(key, value)?.map(Into::into)
    }

    #[fehler::throws(anyhow::Error)]
    pub fn get(&self, key: K) -> Option<V> {
        self.inner.get(key)?.map(Into::into)
    }

    #[fehler::throws(anyhow::Error)]
    pub fn contains_key(&self, key: K) -> bool {
        self.inner.contains_key(key)?
    }

    #[fehler::throws(anyhow::Error)]
    pub fn remove(&self, key: K) -> Option<V> {
        self.inner.remove(key)?.map(Into::into)
    }

    #[fehler::throws(anyhow::Error)]
    pub fn compare_and_swap(
        &self,
        key: K,
        old: Option<&'value V>,
        new: Option<&'value V>,
    ) -> Result<(), CompareAndSwapError<V>> {
        let old: Option<sled::IVec> = old.map(Into::into);
        let new: Option<sled::IVec> = new.map(Into::into);
        self.inner.compare_and_swap(key, old, new)?.map_err(
            |sled::CompareAndSwapError { current, proposed }| CompareAndSwapError {
                current: current.map(Into::into),
                proposed: proposed.map(Into::into),
            },
        )
    }

    #[fehler::throws(anyhow::Error)]
    pub fn fetch_and_update<F: FnMut(Option<V>) -> Option<V>>(
        &self,
        key: K,
        mut f: F,
    ) -> Option<V> {
        self.inner
            .fetch_and_update::<K, sled::IVec, _>(key, move |current_value| {
                f(current_value
                    .map(|current_value| <sled::IVec as From<&[u8]>>::from(current_value).into()))
                .map(Into::into)
            })?
            .map(Into::into)
    }

    #[fehler::throws(anyhow::Error)]
    pub fn update_and_fetch<F: FnMut(Option<V>) -> Option<V>>(
        &self,
        key: K,
        mut f: F,
    ) -> Option<V> {
        self.inner
            .update_and_fetch::<K, sled::IVec, _>(key, move |current_value| {
                f(current_value
                    .map(|current_value| <sled::IVec as From<&[u8]>>::from(current_value).into()))
                .map(Into::into)
            })?
            .map(Into::into)
    }

    pub fn transaction<A, E, F>(&self, f: F) -> sled::transaction::TransactionResult<A, E>
    where
        F: Fn(TransactionalTree<K, V>) -> ConflictableTransactionResult<A, E>,
    {
        self.inner.transaction(move |inner: &sled::transaction::TransactionalTree| {
            f(TransactionalTree::<K, V>::new(inner))
        })
    }

    #[fehler::throws(anyhow::Error)]
    pub fn apply_batch(&self, batch: Batch<K, V>) {
        self.inner.apply_batch(batch.0)?
    }

    pub fn len(&self) -> usize {
        self.inner.len()
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    #[fehler::throws(anyhow::Error)]
    pub fn clear(&self) {
        self.inner.clear()?
    }

    #[fehler::throws(anyhow::Error)]
    pub fn name(&self) -> String {
        std::str::from_utf8(self.inner.name().as_ref())?.to_string()
    }

    #[fehler::throws(anyhow::Error)]
    pub fn checksum(&self) -> u32 {
        self.inner.checksum()?
    }

    #[fehler::throws(anyhow::Error)]
    pub fn flush(&self) -> usize {
        self.inner.flush()?
    }

    #[fehler::throws(anyhow::Error)]
    pub async fn flush_async(&self) -> usize
    where
        K: Send + Sync,
        V: Send + Sync,
    {
        self.inner.flush_async().await?
    }

    pub fn iter(&self) -> Iter<K, V> {
        Iter::new(self.inner.iter())
    }

    pub fn range<R>(&self, range: R) -> Iter<K, V>
    where
        R: core::ops::RangeBounds<K>,
    {
        Iter::new(self.inner.range(range))
    }
}
