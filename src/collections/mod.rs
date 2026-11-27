//! Collections: lists that grow (issue 139). This is a modern extension,
//! not BBC BASIC.
//!
//! A collection lives in a store of the interpreter. A program holds its
//! handle, a whole number above 0, in a normal numeric variable, as it holds
//! the handle of a file channel. An element is a number or a string. A
//! handle is a number, so a list can hold the handle of a collection.

use crate::error::{BBCBasicError, Result};
use std::collections::HashMap;

/// One element of a collection: a number or a string.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Number(f64),
    Text(String),
}

/// One collection in the store. Issue 140 adds maps.
#[derive(Debug, Clone, PartialEq)]
enum Collection {
    List(Vec<Element>),
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

    fn list(&self, handle: i32) -> Result<&Vec<Element>> {
        match self.collections.get(&handle) {
            Some(Collection::List(list)) => Ok(list),
            None => Err(BBCBasicError::BadCollection),
        }
    }

    fn list_mut(&mut self, handle: i32) -> Result<&mut Vec<Element>> {
        match self.collections.get_mut(&handle) {
            Some(Collection::List(list)) => Ok(list),
            None => Err(BBCBasicError::BadCollection),
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
}
