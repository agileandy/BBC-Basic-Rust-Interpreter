//! Collections: lists that grow (issue 139) and maps from a text key to a
//! value (issue 140). This is a modern extension,
//! not BBC BASIC.
//!
//! A collection lives in a store of the interpreter. A program holds its
//! handle, a whole number above 0, in a normal numeric variable, as it holds
//! the handle of a file channel. An element is a number or a string. A
//! handle is a number, so a list can hold the handle of a collection.
//!
//! The conversion between JSON text and collections (issue 142) needs 3
//! more kinds of element: a nested collection, a boolean and null. A
//! getter gives each of them as a number: the handle, -1 or 0, and 0.

use crate::error::{BBCBasicError, Result};
use indexmap::IndexMap;
use serde_json::{Map, Value};
use std::collections::HashMap;

/// The maximum count of collections, one in the other, that `to_json`
/// writes. It is the same as the limit of the JSON parser, so each
/// converted document can go back to JSON text.
pub const MAX_JSON_DEPTH: usize = 128;

/// One element of a collection.
#[derive(Debug, Clone, PartialEq)]
pub enum Element {
    Number(f64),
    Text(String),
    /// A nested list or map, with its handle (issue 142).
    Collection(i32),
    /// A JSON `true` or `false` (issue 142).
    Boolean(bool),
    /// A JSON `null` (issue 142).
    Null,
}

impl Element {
    /// The element as a number: a handle, -1 for true, 0 for false or null.
    /// A string gives Type mismatch.
    fn number(&self) -> Result<f64> {
        match self {
            Element::Number(value) => Ok(*value),
            Element::Collection(handle) => Ok(f64::from(*handle)),
            Element::Boolean(true) => Ok(-1.0),
            Element::Boolean(false) | Element::Null => Ok(0.0),
            Element::Text(_) => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The element as a string. Each other kind gives Type mismatch.
    fn text(&self) -> Result<String> {
        match self {
            Element::Text(text) => Ok(text.clone()),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }
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
        self.get(handle, index)?.number()
    }

    /// The string at an index of a list. A number gives Type mismatch.
    pub fn get_text(&self, handle: i32, index: i32) -> Result<String> {
        self.get(handle, index)?.text()
    }

    /// The kind of the element at an index of a list, for `LISTKIND$`.
    pub fn list_kind(&self, handle: i32, index: i32) -> Result<&'static str> {
        self.kind(self.get(handle, index)?)
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
        self.map_get(handle, key)?.number()
    }

    /// The string of a key of a map. A number gives Type mismatch.
    pub fn map_get_text(&self, handle: i32, key: &str) -> Result<String> {
        self.map_get(handle, key)?.text()
    }

    /// The kind of the value of a key of a map, for `MAPKIND$`.
    pub fn map_kind(&self, handle: i32, key: &str) -> Result<&'static str> {
        self.kind(self.map_get(handle, key)?)
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

    /// The element for a nested collection, for `LISTADDCOL` and
    /// `MAPPUTCOL`. A handle that is not a collection gives Bad collection.
    pub fn collection_element(&self, handle: i32) -> Result<Element> {
        if self.collections.contains_key(&handle) {
            Ok(Element::Collection(handle))
        } else {
            Err(BBCBasicError::BadCollection)
        }
    }

    /// The name of the kind of an element: `number`, `string`, `list`,
    /// `map`, `boolean` or `null`.
    fn kind(&self, element: &Element) -> Result<&'static str> {
        Ok(match element {
            Element::Number(_) => "number",
            Element::Text(_) => "string",
            Element::Boolean(_) => "boolean",
            Element::Null => "null",
            Element::Collection(handle) => match self.collections.get(handle) {
                Some(Collection::List(_)) => "list",
                Some(Collection::Map(_)) => "map",
                None => return Err(BBCBasicError::BadCollection),
            },
        })
    }

    /// `JSONTOLIST`: make a list from a JSON array. Each nested array or
    /// object becomes a new collection. A root that is not an array gives
    /// Type mismatch.
    pub fn list_from_json(&mut self, value: Value) -> Result<i32> {
        match value {
            Value::Array(_) => self.collection_from_json(value),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// `JSONTOMAP`: make a map from a JSON object. Each nested array or
    /// object becomes a new collection. A root that is not an object gives
    /// Type mismatch.
    pub fn map_from_json(&mut self, value: Value) -> Result<i32> {
        match value {
            Value::Object(_) => self.collection_from_json(value),
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// Make a list from an array or a map from an object, and give its handle.
    fn collection_from_json(&mut self, value: Value) -> Result<i32> {
        match value {
            Value::Array(values) => {
                let handle = self.new_list()?;
                let list = values
                    .into_iter()
                    .map(|value| self.element_from_json(value))
                    .collect::<Result<Vec<_>>>()?;
                *self.list_mut(handle)? = list;
                Ok(handle)
            }
            Value::Object(members) => {
                let handle = self.new_map()?;
                let map = members
                    .into_iter()
                    .map(|(key, value)| Ok((key, self.element_from_json(value)?)))
                    .collect::<Result<IndexMap<_, _>>>()?;
                *self.map_mut(handle)? = map;
                Ok(handle)
            }
            _ => Err(BBCBasicError::TypeMismatch),
        }
    }

    /// The element for one JSON value. An array or an object becomes a new
    /// collection.
    fn element_from_json(&mut self, value: Value) -> Result<Element> {
        Ok(match value {
            Value::Number(number) => {
                Element::Number(number.as_f64().ok_or(BBCBasicError::TypeMismatch)?)
            }
            Value::String(text) => Element::Text(text),
            Value::Bool(value) => Element::Boolean(value),
            Value::Null => Element::Null,
            nested => Element::Collection(self.collection_from_json(nested)?),
        })
    }

    /// `TOJSON$`: the JSON value of a list or a map. A `Collection` element
    /// becomes a nested array or object. A number is a number, also if it is
    /// a handle. A collection in itself, or more than `MAX_JSON_DEPTH`
    /// collections one in the other, gives Bad collection. More than
    /// `budget` elements gives String too long, because each element is at
    /// least 1 character of text.
    pub fn to_json(&self, handle: i32, budget: usize) -> Result<Value> {
        let mut writer = JsonWriter {
            store: self,
            open: Vec::new(),
            count: 0,
            budget,
        };
        writer.collection(handle)
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

/// The state of one `to_json` call. `open` holds the handles of the
/// collections that are written now, from the root, so that a collection
/// in itself gives an error and does not loop.
struct JsonWriter<'a> {
    store: &'a CollectionStore,
    open: Vec<i32>,
    count: usize,
    budget: usize,
}

impl JsonWriter<'_> {
    fn collection(&mut self, handle: i32) -> Result<Value> {
        if self.open.contains(&handle) || self.open.len() >= MAX_JSON_DEPTH {
            return Err(BBCBasicError::BadCollection);
        }
        self.open.push(handle);
        let value = match self.store.collections.get(&handle) {
            Some(Collection::List(list)) => Value::Array(
                list.iter()
                    .map(|element| self.element(element))
                    .collect::<Result<_>>()?,
            ),
            Some(Collection::Map(map)) => Value::Object(
                map.iter()
                    .map(|(key, element)| Ok((key.clone(), self.element(element)?)))
                    .collect::<Result<Map<_, _>>>()?,
            ),
            None => return Err(BBCBasicError::BadCollection),
        };
        self.open.pop();
        Ok(value)
    }

    fn element(&mut self, element: &Element) -> Result<Value> {
        self.count += 1;
        if self.count > self.budget {
            return Err(BBCBasicError::StringTooLong);
        }
        Ok(match element {
            Element::Number(value) => crate::json::number_value(*value)?,
            Element::Text(text) => Value::String(text.clone()),
            Element::Collection(handle) => self.collection(*handle)?,
            Element::Boolean(value) => Value::Bool(*value),
            Element::Null => Value::Null,
        })
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

    fn json(text: &str) -> Value {
        serde_json::from_str(text).unwrap()
    }

    #[test]
    fn test_the_getters_give_a_collection_a_boolean_and_null_as_numbers() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        let inner = store.new_map().unwrap();
        let element = store.collection_element(inner).unwrap();
        store.add(list, element).unwrap();
        store.add(list, Element::Boolean(true)).unwrap();
        store.add(list, Element::Boolean(false)).unwrap();
        store.add(list, Element::Null).unwrap();

        assert_eq!(store.get_number(list, 0), Ok(f64::from(inner)));
        assert_eq!(store.get_number(list, 1), Ok(-1.0));
        assert_eq!(store.get_number(list, 2), Ok(0.0));
        assert_eq!(store.get_number(list, 3), Ok(0.0));
        for index in 0..4 {
            assert_eq!(store.is_text(list, index), Ok(false));
            assert_eq!(
                store.get_text(list, index),
                Err(BBCBasicError::TypeMismatch)
            );
        }

        let map = store.new_map().unwrap();
        store.put(map, "B", Element::Boolean(true)).unwrap();
        store.put(map, "Z", Element::Null).unwrap();
        assert_eq!(store.map_get_number(map, "B"), Ok(-1.0));
        assert_eq!(store.map_get_number(map, "Z"), Ok(0.0));
        assert_eq!(store.map_is_text(map, "B"), Ok(false));
    }

    #[test]
    fn test_collection_element_of_a_handle_that_does_not_exist_gives_bad_collection() {
        let store = CollectionStore::new();

        assert_eq!(
            store.collection_element(7),
            Err(BBCBasicError::BadCollection)
        );
    }

    #[test]
    fn test_list_from_json_makes_a_collection_for_each_nested_array_and_object() {
        let mut store = CollectionStore::new();
        let list = store
            .list_from_json(json(r#"[1,"A",[2],{"K":true},false,null]"#))
            .unwrap();

        assert_eq!(store.length(list), Ok(6));
        let kinds: Vec<_> = (0..6).map(|i| store.list_kind(list, i).unwrap()).collect();
        assert_eq!(
            kinds,
            ["number", "string", "list", "map", "boolean", "null"]
        );
        let inner = store.get_number(list, 2).unwrap() as i32;
        assert_eq!(store.get_number(inner, 0), Ok(2.0));
        let map = store.get_number(list, 3).unwrap() as i32;
        assert_eq!(store.map_kind(map, "K"), Ok("boolean"));
    }

    #[test]
    fn test_a_root_of_the_wrong_type_gives_type_mismatch() {
        let mut store = CollectionStore::new();
        let mismatch = Err(BBCBasicError::TypeMismatch);

        assert_eq!(store.list_from_json(json(r#"{"A":1}"#)), mismatch);
        assert_eq!(store.list_from_json(json("3")), mismatch);
        assert_eq!(store.map_from_json(json("[1]")), mismatch);
        assert_eq!(store.map_from_json(json(r#""A""#)), mismatch);
    }

    #[test]
    fn test_to_json_of_a_converted_document_gives_the_same_value() {
        let mut store = CollectionStore::new();
        let text = r#"{"n":1.5,"w":-2,"s":"Q\"","a":[[],{},true,null],"o":{"x":false}}"#;
        let map = store.map_from_json(json(text)).unwrap();

        assert_eq!(store.to_json(map, 1000), Ok(json(text)));
    }

    #[test]
    fn test_to_json_writes_a_number_that_is_a_handle_as_a_number() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        let inner = store.new_list().unwrap();
        store.add(list, number(f64::from(inner))).unwrap();
        let element = store.collection_element(inner).unwrap();
        store.add(list, element).unwrap();

        let expected = json(&format!("[{},[]]", inner));
        assert_eq!(store.to_json(list, 1000), Ok(expected));
    }

    #[test]
    fn test_to_json_of_a_list_that_holds_itself_gives_bad_collection() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        let inner = store.new_list().unwrap();
        let element = store.collection_element(inner).unwrap();
        store.add(list, element).unwrap();
        let element = store.collection_element(list).unwrap();
        store.add(inner, element).unwrap();

        let bad = Err(BBCBasicError::BadCollection);
        assert_eq!(store.to_json(list, 1000), bad);
    }

    #[test]
    fn test_to_json_can_write_one_collection_2_times() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        let inner = store.new_list().unwrap();
        for _ in 0..2 {
            let element = store.collection_element(inner).unwrap();
            store.add(list, element).unwrap();
        }

        assert_eq!(store.to_json(list, 1000), Ok(json("[[],[]]")));
    }

    #[test]
    fn test_to_json_deeper_than_the_limit_gives_bad_collection() {
        let mut store = CollectionStore::new();
        let chain: Vec<i32> = (0..=MAX_JSON_DEPTH)
            .map(|_| store.new_list().unwrap())
            .collect();
        for pair in chain.windows(2) {
            let element = store.collection_element(pair[1]).unwrap();
            store.add(pair[0], element).unwrap();
        }

        let bad = Err(BBCBasicError::BadCollection);
        assert_eq!(store.to_json(chain[0], 100_000), bad);
        assert!(store.to_json(chain[1], 100_000).is_ok());
    }

    #[test]
    fn test_to_json_with_more_elements_than_the_budget_gives_string_too_long() {
        let mut store = CollectionStore::new();
        let list = store.new_list().unwrap();
        for _ in 0..10 {
            store.add(list, number(1.0)).unwrap();
        }

        let too_long = Err(BBCBasicError::StringTooLong);
        assert_eq!(store.to_json(list, 9), too_long);
        assert!(store.to_json(list, 11).is_ok());
    }
}
