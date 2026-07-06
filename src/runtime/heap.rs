use crate::runtime::Value;

#[derive(Clone)]
pub enum HeapObj {
    String(String),
    Vec(Vec<Value>),
    Value(Value),
}

#[derive(Debug, Clone, Copy)]
pub struct ObjectId(usize);

pub trait Heap {
    fn alloc_obj(&mut self, obj: HeapObj) -> ObjectId;
    fn fetch_obj(&self, id: ObjectId) -> &HeapObj;
    fn clone_obj(&mut self, id: ObjectId) -> ObjectId;
}

#[derive(Default)]
pub struct SimpleHeap {
    arena: Vec<HeapObj>,
}

impl SimpleHeap {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Heap for SimpleHeap {
    fn alloc_obj(&mut self, obj: HeapObj) -> ObjectId {
        self.arena.push(obj);
        ObjectId(self.arena.len() - 1)
    }

    fn fetch_obj(&self, id: ObjectId) -> &HeapObj {
        &self.arena[id.0]
    }

    fn clone_obj(&mut self, id: ObjectId) -> ObjectId {
        let obj = self.fetch_obj(id).clone();
        self.alloc_obj(obj)
    }
}
