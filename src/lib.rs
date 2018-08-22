#![no_std]
#![cfg_attr(not(feature = "std"), feature(alloc))]

#[macro_use]
extern crate cfg_if;

cfg_if! {
    if #[cfg(feature = "std")] {
        extern crate std;
        use std::vec::{self, Vec};
    } else {
        extern crate alloc;
        use alloc::vec::{self, Vec};
    }
}

use core::mem;

#[derive(Clone, Debug)]
pub struct Arena<T> {
    items: Vec<Entry<T>>,
    generation: u64,
    free_list_head: Option<usize>,
}

#[derive(Clone, Debug)]
enum Entry<T> {
    Free { next_free: Option<usize> },
    Occupied { generation: u64, value: T },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Index {
    index: usize,
    generation: u64,
}

const DEFAULT_CAPACITY: usize = 4;

impl<T> Arena<T> {
    pub fn new() -> Arena<T> {
        Arena::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(n: usize) -> Arena<T> {
        assert!(n > 0);
        let mut arena = Arena {
            items: Vec::new(),
            generation: 0,
            free_list_head: None,
        };
        arena.reserve(n);
        arena
    }

    pub fn try_insert(&mut self, value: T) -> Result<Index, T> {
        match self.free_list_head {
            None => Err(value),
            Some(i) => match self.items[i] {
                Entry::Occupied { .. } => panic!("corrupt free list"),
                Entry::Free { next_free } => {
                    self.free_list_head = next_free;
                    self.items[i] = Entry::Occupied {
                        generation: self.generation,
                        value,
                    };
                    Ok(Index {
                        index: i,
                        generation: self.generation,
                    })
                }
            },
        }
    }

    pub fn insert(&mut self, value: T) -> Index {
        match self.try_insert(value) {
            Ok(i) => i,
            Err(value) => {
                let len = self.items.len();
                self.reserve(len);
                self.try_insert(value)
                    .map_err(|_| ())
                    .expect("inserting will always succeed after reserving additional space")
            }
        }
    }

    pub fn remove(&mut self, i: Index) -> Option<T> {
        assert!(i.index < self.items.len());
        let entry = mem::replace(
            &mut self.items[i.index],
            Entry::Free {
                next_free: self.free_list_head,
            },
        );
        match entry {
            Entry::Occupied { generation, value } => if generation == i.generation {
                self.generation += 1;
                self.free_list_head = Some(i.index);
                Some(value)
            } else {
                self.items[i.index] = Entry::Occupied { generation, value };
                None
            },
            e @ Entry::Free { .. } => {
                self.items[i.index] = e;
                None
            }
        }
    }

    pub fn contains(&self, i: Index) -> bool {
        self.get(i).is_some()
    }

    pub fn get(&self, i: Index) -> Option<&T> {
        assert!(i.index < self.items.len());
        match self.items[i.index] {
            Entry::Occupied {
                generation,
                ref value,
            }
                if generation == i.generation =>
            {
                Some(value)
            }
            _ => None,
        }
    }

    pub fn get_mut(&mut self, i: Index) -> Option<&mut T> {
        assert!(i.index < self.items.len());
        match self.items[i.index] {
            Entry::Occupied {
                generation,
                ref mut value,
            }
                if generation == i.generation =>
            {
                Some(value)
            }
            _ => None,
        }
    }

    pub fn capacity(&self) -> usize {
        self.items.len()
    }

    pub fn reserve(&mut self, additional_capacity: usize) {
        let start = self.items.len();
        let end = self.items.len() + additional_capacity;
        let old_head = self.free_list_head;
        self.items.reserve_exact(additional_capacity);
        self.items.extend((start..end).map(|i| {
            if i == end - 1 {
                Entry::Free {
                    next_free: old_head,
                }
            } else {
                Entry::Free {
                    next_free: Some(i + 1),
                }
            }
        }));
        self.free_list_head = Some(start);
    }
}

impl<T> IntoIterator for Arena<T> {
    type Item = T;
    type IntoIter = IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        IntoIter {
            inner: self.items.into_iter()
        }
    }
}

pub struct IntoIter<T> {
    inner: vec::IntoIter<Entry<T>>,
}

impl<T> Iterator for IntoIter<T> {
    type Item = T;

    fn next(&mut self) -> Option<Self::Item> {
        loop {
            match self.inner.next() {
                Some(Entry::Free { .. }) => continue,
                Some(Entry::Occupied { value, .. }) => return Some(value),
                None => return None,
            }
        }
    }
}
