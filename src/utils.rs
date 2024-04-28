use std::ops::{Deref, DerefMut};

pub fn multiple_of(mut value: u32, multiple: u32) -> u32 {
    let remainder = value % multiple;
    if remainder != 0 {
        value += multiple - remainder;
    }

    value
}

/// A type thats assumed to exist when accessed
pub enum Exists<T> {
    Some(T),
    None,
}

impl<T> Exists<T> {
    pub fn is_none(&self) -> bool {
        matches!(self, Self::None)
    }
}

impl<T> Deref for Exists<T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        match self {
            Exists::Some(v) => v,
            Exists::None => panic!("Type expected to exist by now but didn't"),
        }
    }
}

impl<T> DerefMut for Exists<T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        match self {
            Exists::Some(v) => v,
            Exists::None => panic!("Type expected to exist by now but didn't"),
        }
    }
}
