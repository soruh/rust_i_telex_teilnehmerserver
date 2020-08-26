pub struct Iter<K, V> {
    inner: sled::Iter,
    _phantom: core::marker::PhantomData<(K, V)>,
}

impl<K, V> Iter<K, V> {
    pub(super) const fn new(inner: sled::Iter) -> Self {
        Self { inner, _phantom: core::marker::PhantomData }
    }
}

impl<'value, K, V> Iter<K, V>
where
    K: AsRef<[u8]> + From<sled::IVec>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    pub fn keys(self) -> impl DoubleEndedIterator<Item = anyhow::Result<K>> {
        self.map(|r| r.map(|(k, _v)| k))
    }

    pub fn values(self) -> impl DoubleEndedIterator<Item = anyhow::Result<V>> {
        self.map(|r| r.map(|(_k, v)| v))
    }
}

impl<'value, K, V> Iterator for Iter<K, V>
where
    K: AsRef<[u8]> + From<sled::IVec>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    type Item = anyhow::Result<(K, V)>;

    fn next(&mut self) -> Option<Self::Item> {
        let item = self.inner.next()?;

        Some(item.map(|(k, v)| (k.into(), v.into())).map_err(|err| err.into()))
    }
}

impl<'value, K, V> DoubleEndedIterator for Iter<K, V>
where
    K: AsRef<[u8]> + From<sled::IVec>,
    V: From<sled::IVec> + Into<sled::IVec>,
    &'value V: Into<sled::IVec> + 'value,
{
    fn next_back(&mut self) -> Option<Self::Item> {
        let item = self.inner.next()?;

        Some(item.map(|(k, v)| (k.into(), v.into())).map_err(|err| err.into()))
    }
}
