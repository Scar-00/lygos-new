//! Append-only, typed-index storage. Entries are never removed or reordered.
//!
//! IDs from different arenas cannot be accidentally interchanged:
//! ```compile_fail
//! use lygos_ast::arena::{Arena, ExprId, StmtId};
//! let expressions = Arena::<i32, ExprId>::default();
//! let mut statements = Arena::<i32, StmtId>::default();
//! let statement = statements.insert(1);
//! expressions.get(statement);
//! ```
use std::{marker::PhantomData, ops::Index};

pub trait ArenaId: Copy {
    fn from_index(index: u32) -> Self;
    fn index(self) -> usize;
}

macro_rules! ids {
    ($($name:ident),* $(,)?) => {$(
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub struct $name(u32);
        impl ArenaId for $name {
            fn from_index(index: u32) -> Self { Self(index) }
            fn index(self) -> usize { self.0 as usize }
        }
    )*};
}
ids!(
    ExprId, StmtId, BlockId, TypeId, SymbolId, ScopeId, FunctionId
);

#[derive(Debug)]
pub struct Arena<T, I> {
    entries: Vec<T>,
    id: PhantomData<fn() -> I>,
}
impl<T, I> Default for Arena<T, I> {
    fn default() -> Self {
        Self {
            entries: Vec::new(),
            id: PhantomData,
        }
    }
}
impl<T, I: ArenaId> Arena<T, I> {
    pub fn insert(&mut self, value: T) -> I {
        let id =
            I::from_index(u32::try_from(self.entries.len()).expect("arena exceeds u32 ID space"));
        self.entries.push(value);
        id
    }
    pub fn get(&self, id: I) -> Option<&T> {
        self.entries.get(id.index())
    }
    pub fn get_mut(&mut self, id: I) -> Option<&mut T> {
        self.entries.get_mut(id.index())
    }
    pub fn len(&self) -> usize {
        self.entries.len()
    }
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub fn iter(&self) -> impl Iterator<Item = (I, &T)> {
        self.entries
            .iter()
            .enumerate()
            .map(|(i, value)| (I::from_index(i as u32), value))
    }
}
impl<T, I: ArenaId> Index<I> for Arena<T, I> {
    type Output = T;
    fn index(&self, id: I) -> &T {
        self.get(id).expect("ID does not belong to this arena")
    }
}
