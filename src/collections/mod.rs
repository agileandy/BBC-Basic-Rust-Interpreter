//! Collections: lists that grow (issue 139) and maps from a text key to a
//! value (issue 140). This is a modern extension,
//! not BBC BASIC.
//!
//! A collection lives in a store of the interpreter. A program holds its
//! handle, a whole number above 0, in a normal numeric variable, as it holds
//! the handle of a file channel. An element is a number or a string. A
//! handle is a number, so a list can hold the handle of a collection.

use crate::error::{BBCBasicError, Result};
use indexmap::IndexMap;
use std::collections::HashMap;

/// One element of a collection: a number or a string.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Number(f64),
    Text(String),
}

/// One collection in the store: a list (issue 139) or a map (issue 140).
/// The keys of a map stay in the order in which they were put.
#[derive(Debug, Clone, PartialEq)]
enum Collection {
    List(Vec<Element>),
    Map(IndexMap<String, Element>),
}

/// The collections of a program, found by handle. A handle is never used
/// 2 times in one store, so an old handle cannot find a new collection.
#[derive(Debug)]
pub struct CollectionStore {
    collections: HashMap<i32, Collection>,
    next_handle: i32,
}

impl Default for CollectionStore {
    fn default() -> Self {
        Self::new()
    }
}

impl CollectionStore {
    /// Make an empty store.
    pub fn new() -> Self {
        Self {
            collections: HashMap::new(),
            next_handle: 1,
        }
    }

    /// Remove all collections. RUN, NEW and CLEAR use this. The handles
    /// that the store gave before are not used again.
    pub fn clear(&mut self) {
        self.collections.clear();
    }

    /// Make an empty list and give its handle. If no handle is free, give No room.
    pub fn new_list(&mut self) -> Result<i32> {
        let handle = self.next_handle;
        self.next_handle = handle.checked_add(1).ok_or(BBCBasicError::NoRoom)?;
        self.collections
            .insert(handle, Collection::List(Vec::new()));
        Ok(handle)
    }

    /// The count of elements in a list.
    pub fn length(&self, handle: i32) -> Result<usize> {
        Ok(self.list(handle)?.len())
    }

    /// The element at an index (from 0) of a list.
    pub fn get(&self, handle: i32, index: i32) -> Result<&Element> {
        let list = self.list(handle)?;
        Ok(&list[element_index(index, list.len())?])
    }

    /// The number at an index of a list. A string gives Type mismatch.
    pub fn get_number(&self, handle: i32, index: i32) -> Result<f64> {
        match self.get(handle, index)? {
            Element::Number(value) => Ok(*value),
            Element::Text(_) => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The string at an index of a list. A number gives Type mismatch.
    pub fn get_text(&self, handle: i32, index: i32) -> Result<String> {
        match self.get(handle, index)? {
            Element::Text(text) => Ok(text.clone()),
            Element::Number(_) => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// Tell if the element at an index of a list is a string.
    pub fn is_text(&self, handle: i32, index: i32) -> Result<bool> {
        Ok(matches!(self.get(handle, index)?, Element::Text(_)))
    }

    /// Add an element at the end of a list.
    pub fn add(&mut self, handle: i32, element: Element) -> Result<()> {
        self.list_mut(handle)?.push(element);
        Ok(())
    }

    /// Change the element at an index of a list.
    pub fn set(&mut self, handle: i32, index: i32, element: Element) -> Result<()> {
        let list = self.list_mut(handle)?;
        let index = element_index(index, list.len())?;
        list[index] = element;
        Ok(())
    }

    /// Put an element before the index. The index can be the count of
    /// elements: then the element goes at the end.
    pub fn insert(&mut self, handle: i32, index: i32, element: Element) -> Result<()> {
        let list = self.list_mut(handle)?;
        let index = element_index(index, list.len() + 1)?;
        list.insert(index, element);
        Ok(())
    }

    /// Remove the element at an index of a list.
    pub fn remove(&mut self, handle: i32, index: i32) -> Result<()> {
        let list = self.list_mut(handle)?;
        let index = element_index(index, list.len())?;
        list.remove(index);
        Ok(())
    }

    /// Remove all elements of a list. The list stays.
    pub fn clear_list(&mut self, handle: i32) -> Result<()> {
        self.list_mut(handle)?.clear();
        Ok(())
    }

    /// Make an empty map and give its handle. If no handle is free, give No room.
    pub fn new_map(&mut self) -> Result<i32> {
        let handle = self.next_handle;
        self.next_handle = handle.checked_add(1).ok_or(BBCBasicError::NoRoom)?;
        self.collections
            .insert(handle, Collection::Map(IndexMap::new()));
        Ok(handle)
    }

    /// The count of keys in a map.
    pub fn map_length(&self, handle: i32) -> Result<usize> {
        Ok(self.map(handle)?.len())
    }

    /// The key at an index (from 0) of a map, in the order of the puts.
    pub fn map_key(&self, handle: i32, index: i32) -> Result<String> {
        let map = self.map(handle)?;
        let index = element_index(index, map.len())?;
        Ok(map.get_index(index).expect("index is checked").0.clone())
    }

    /// The value of a key of a map. A key that is not there gives No such key.
    pub fn map_get(&self, handle: i32, key: &str) -> Result<&Element> {
        self.map(handle)?
            .get(key)
            .ok_or_else(|| BBCBasicError::NoSuchKey(key.to_string()))
    }

    /// The number of a key of a map. A string gives Type mismatch.
    pub fn map_get_number(&self, handle: i32, key: &str) -> Result<f64> {
        match self.map_get(handle, key)? {
            Element::Number(value) => Ok(*value),
            Element::Text(_) => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The string of a key of a map. A number gives Type mismatch.
    pub fn map_get_text(&self, handle: i32, key: &str) -> Result<String> {
        match self.map_get(handle, key)? {
            Element::Text(text) => Ok(text.clone()),
            Element::Number(_) => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// Tell if the value of a key of a map is a string.
    pub fn map_is_text(&self, handle: i32, key: &str) -> Result<bool> {
        Ok(matches!(self.map_get(handle, key)?, Element::Text(_)))
    }

    /// Put a value for a key of a map. If the key is there, replace its
    /// value and keep the place of the key.
    pub fn put(&mut self, handle: i32, key: &str, element: Element) -> Result<()> {
        self.map_mut(handle)?.insert(key.to_string(), element);
        Ok(())
    }

    /// Tell if a map has a key.
    pub fn has(&self, handle: i32, key: &str) -> Result<bool> {
        Ok(self.map(handle)?.contains_key(key))
    }

    /// Remove a key of a map. The other keys keep their order. A key that
    /// is not there gives No such key.
    pub fn remove_key(&mut self, handle: i32, key: &str) -> Result<()> {
        self.map_mut(handle)?
            .shift_remove(key)
            .map(|_| ())
            .ok_or_else(|| BBCBasicError::NoSuchKey(key.to_string()))
    }

    /// Remove all keys of a map. The map stays.
    pub fn clear_map(&mut self, handle: i32) -> Result<()> {
        self.map_mut(handle)?.clear();
        Ok(())
    }

    fn map(&self, handle: i32) -> Result<&IndexMap<String, Element>> {
        match self.collections.get(&handle) {
            Some(Collection::Map(map)) => Ok(map),
            _ => Err(BBCBasicError::BadCollection),
        }
    }

    fn map_mut(&mut self, handle: i32) -> Result<&mut IndexMap<String, Element>> {
        match self.collections.get_mut(&handle) {
            Some(Collection::Map(map)) => Ok(map),
            _ => Err(BBCBasicError::BadCollection),
        }
    }

    fn list(&self, handle: i32) -> Result<&Vec<Element>> {
        match self.collections.get(&handle) {
            Some(Collection::List(list)) => Ok(list),
            _ => Err(BBCBasicError::BadCollection),
        }
    }

    fn list_mut(&mut self, handle: i32) -> Result<&mut Vec<Element>> {
        match self.collections.get_mut(&handle) {
            Some(Collection::List(list)) => Ok(list),
            _ => Err(BBCBasicError::BadCollection),
        }
    }
}

/// Convert an index of a program to a Rust index below `limit`. An index
/// out of range gives Subscript out of range.
fn element_index(index: i32, limit: usize) -> Result<usize> {
    usize::try_from(index)
        .ok()
        .filter(|&index| index < limit)
        .ok_or(BBCBasicError::SubscriptOutOfRange)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn number(value: f64) -> Element {
        Element::Number(value)
    }

    fn text(value: &str) -> Element {
        Element::Text(value.to_string())
    }

    #[test]
    fn test_new_list_gives_a_different_handle_above_0_each_time() {
        let mut store = CollectionStore::new();
        let first = store.new_list().unwrap();
        let second = store.new_list().unwrap();

        assert!(first > 0);
        assert_ne!(first, second);
        assert_eq!(store.length(first), Ok(0));
    }

    #[test]
    fn test_add_get_and_length_with_numbers_and_strings() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        store.add(list, number(5.0)).unwrap();
        store.add(list, text("CAT")).unwrap();

        assert_eq!(store.length(list), Ok(2));
        assert_eq!(store.get_number(list, 0), Ok(5.0));
        assert_eq!(store.get_text(list, 1), Ok("CAT".to_string()));
        assert_eq!(store.is_text(list, 0), Ok(false));
        assert_eq!(store.is_text(list, 1), Ok(true));
    }

    #[test]
    fn test_the_wrong_getter_gives_type_mismatch() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        store.add(list, number(5.0)).unwrap();
        store.add(list, text("CAT")).unwrap();

        assert_eq!(store.get_text(list, 0), Err(BBCBasicError::TypeMismatch));
        assert_eq!(store.get_number(list, 1), Err(BBCBasicError::TypeMismatch));
    }

    #[test]
    fn test_set_insert_remove_and_clear_list() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        store.add(list, number(2.0)).unwrap();
        store.insert(list, 0, number(1.0)).unwrap();
        store.insert(list, 2, number(3.0)).unwrap();
        store.set(list, 1, text("TWO")).unwrap();

        assert_eq!(store.get(list, 0), Ok(&number(1.0)));
        assert_eq!(store.get(list, 1), Ok(&text("TWO")));
        assert_eq!(store.get(list, 2), Ok(&number(3.0)));

        store.remove(list, 0).unwrap();
        assert_eq!(store.get(list, 0), Ok(&text("TWO")));
        assert_eq!(store.length(list), Ok(2));

        store.clear_list(list).unwrap();
        assert_eq!(store.length(list), Ok(0));
    }

    #[test]
    fn test_an_index_out_of_range_gives_subscript_out_of_range() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        store.add(list, number(1.0)).unwrap();
        let out_of_range = Err(BBCBasicError::SubscriptOutOfRange);

        assert_eq!(store.get(list, 1).map(|_| ()), out_of_range);
        assert_eq!(store.get(list, -1).map(|_| ()), out_of_range);
        assert_eq!(store.set(list, 1, number(0.0)), out_of_range);
        assert_eq!(store.insert(list, 2, number(0.0)), out_of_range);
        assert_eq!(store.remove(list, 1), out_of_range);
        assert_eq!(store.length(list), Ok(1));
    }

    #[test]
    fn test_a_handle_that_is_not_a_list_gives_bad_collection() {
        let mut store = CollectionStore::new();
        let bad = Err(BBCBasicError::BadCollection);

        assert_eq!(store.length(0).map(|_| ()), bad);
        assert_eq!(store.add(1, number(0.0)), bad);
        assert_eq!(store.clear_list(-3), bad);
    }

    #[test]
    fn test_clear_removes_all_lists_and_does_not_use_an_old_handle_again() {
        let mut store = CollectionStore::new();
        let old = store.new_list().unwrap();
        store.clear();

        assert_eq!(store.length(old), Err(BBCBasicError::BadCollection));
        let new = store.new_list().unwrap();
        assert_ne!(new, old);
    }

    #[test]
    fn test_map_put_get_and_replace_keep_the_key_order() {
        let mut store = CollectionStore::new();
        let map = store.new_map().unwrap();
        store.put(map, "b", number(1.0)).unwrap();
        store.put(map, "A", text("X")).unwrap();
        store.put(map, "a", number(3.0)).unwrap();
        store.put(map, "b", text("Y")).unwrap();

        assert_eq!(store.map_length(map), Ok(3));
        assert_eq!(store.map_get_text(map, "b"), Ok("Y".to_string()));
        assert_eq!(store.map_get_number(map, "a"), Ok(3.0));
        assert_eq!(store.map_is_text(map, "A"), Ok(true));
        assert_eq!(store.map_key(map, 0), Ok("b".to_string()));
        assert_eq!(store.map_key(map, 1), Ok("A".to_string()));
        assert_eq!(store.map_key(map, 2), Ok("a".to_string()));
    }

    #[test]
    fn test_map_has_remove_and_clear_map() {
        let mut store = CollectionStore::new();
        let map = store.new_map().unwrap();
        for key in ["x", "", "z"] {
            store.put(map, key, number(1.0)).unwrap();
        }

        assert_eq!(store.has(map, ""), Ok(true));
        assert_eq!(store.has(map, "X"), Ok(false));
        store.remove_key(map, "x").unwrap();
        assert_eq!(store.has(map, "x"), Ok(false));
        assert_eq!(store.map_key(map, 0), Ok("".to_string()));
        assert_eq!(store.map_key(map, 1), Ok("z".to_string()));

        store.clear_map(map).unwrap();
        assert_eq!(store.map_length(map), Ok(0));
    }

    #[test]
    fn test_a_key_that_is_not_there_gives_no_such_key() {
        let mut store = CollectionStore::new();
        let map = store.new_map().unwrap();
        let missing = Err(BBCBasicError::NoSuchKey("K".to_string()));

        assert_eq!(store.map_get_number(map, "K").map(|_| ()), missing);
        assert_eq!(store.map_get_text(map, "K").map(|_| ()), missing);
        assert_eq!(store.map_is_text(map, "K").map(|_| ()), missing);
        assert_eq!(store.remove_key(map, "K"), missing);
        assert_eq!(
            store.map_key(map, 0),
            Err(BBCBasicError::SubscriptOutOfRange)
        );
    }

    #[test]
    fn test_a_list_handle_is_not_a_map_and_a_map_handle_is_not_a_list() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        let map = store.new_map().unwrap();
        let bad = Err(BBCBasicError::BadCollection);

        assert_eq!(store.map_length(list).map(|_| ()), bad);
        assert_eq!(store.put(list, "K", number(0.0)), bad);
        assert_eq!(store.has(list, "K").map(|_| ()), bad);
        assert_eq!(store.length(map).map(|_| ()), bad);
        assert_eq!(store.add(map, number(0.0)), bad);
    }
}
