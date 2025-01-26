pub trait DataStore<K, V, E> {
    fn save(&self, id: &K, value: &V) -> Result<(), E>;

    fn load(&self, key: &K) -> Result<Option<V>, E>;

    fn delete(&self, id: &K) -> Result<(), E>;

    fn search(&self, filter: &dyn Filter<V>) -> Result<Vec<V>, E>;
}

pub trait Filter<V> {
    fn pass(&self, v: &V) -> bool;
}
