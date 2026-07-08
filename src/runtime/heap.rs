use crate::runtime::Value;

#[derive(Debug, Clone, Copy)]
pub struct ObjectId(pub(crate) usize);

pub trait Heap {
    fn alloc_obj(&mut self, obj: Value) -> ObjectId;
    fn fetch_obj(&self, id: ObjectId) -> &Value;
    fn update_obj(&mut self, id: ObjectId, obj: Value);
}

#[derive(Default)]
pub struct SimpleHeap {
    pub arena: Vec<Value>,
}

impl SimpleHeap {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Heap for SimpleHeap {
    fn alloc_obj(&mut self, obj: Value) -> ObjectId {
        self.arena.push(obj);
        ObjectId(self.arena.len() - 1)
    }

    fn fetch_obj(&self, id: ObjectId) -> &Value {
        &self.arena[id.0]
    }

    fn update_obj(&mut self, id: ObjectId, obj: Value) {
        self.arena[id.0] = obj;
    }
}
