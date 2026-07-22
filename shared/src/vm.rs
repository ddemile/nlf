use indexmap::IndexMap;

pub type HeapIndex = usize;

#[derive(Clone, Copy, PartialEq, PartialOrd, Debug)]
pub enum Value {
    String(HeapIndex),
    Float(f64),
    Int(u64),
    Bool(bool),
    Closure(HeapIndex),
    Native(HeapIndex),
    Object(HeapIndex),
    Array(HeapIndex),
    Class(HeapIndex),
    Void
}

pub trait AbstractVMContext {
    fn pop_value(&mut self) -> Value;
    fn pop_string_id(&mut self) -> usize;
    fn get_string(&mut self, string_id: usize) -> &str;
    fn pop_float(&mut self) -> f64;
    fn pop_bool(&mut self) -> bool;

    fn push_value(&mut self, value: Value);

    fn stringify(&mut self, value: &Value) -> String;

    fn heap(&mut self) -> &mut dyn AbstractHeap;
}

pub type VMContext<'a> = &'a mut dyn AbstractVMContext;

#[derive(Clone)]
pub struct Object {
    pub map: IndexMap<String, Value>
}

impl Object {
    #[inline(always)]
    pub fn get(&self, key: &str) -> Option<&Value> {
        self.map.get(key)
    }

    #[inline(always)]
    pub fn set(&mut self, key: &str, value: Value) {
        self.map.insert(key.to_string(), value);
    }
}

#[derive(Clone)]
pub struct Array {
    pub vec: Vec<Value>
}

impl Array {
    #[inline(always)]
    pub fn get(&self, index: usize) -> Option<&Value> {
        self.vec.get(index)
    }

    #[inline(always)]
    pub fn set(&mut self, index: usize, value: Value) {
        self.vec[index] = value;
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Class {
    pub static_fields: IndexMap<String, Value>,
    pub constructor_id: HeapIndex,
}

pub trait AbstractHeap {
    fn allocate_string(&mut self, string: String) -> usize;
    fn allocate_object(&mut self, object: Object) -> usize;
    fn allocate_array(&mut self, array: Array) -> usize;
    fn allocate_class(&mut self, class: Class) -> usize;
    fn allocate_native_function(&mut self, native_function: fn(&mut dyn AbstractVMContext)) -> usize;
    
    fn get_string(&mut self, string_id: usize) -> &String;
    fn get_object(&mut self, object_id: usize) -> &Object;
    fn get_array(&mut self, array_id: usize) -> &Array;
    fn get_class(&mut self, class_id: usize) -> &Class;
    fn get_native_function(&mut self, native_function_id: usize) -> fn(&mut dyn AbstractVMContext);
}