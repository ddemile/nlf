
use nlf_macros::module;
use rand::Rng;
use nlf_shared::numbers::{DynamicNumber, NumberHolder};

use crate::{argument, errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder};

module!("random", {
    fn rand(_values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let mut rng = rand::rng();
        let n = rng.random();

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Float32(n))))
    }

    fn randint(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let min = argument!(values, ValueHolder::Number, "min", 0).into();
        let max = argument!(values, ValueHolder::Number, "max", 1).into();

        let mut rng = rand::rng();
        let n = rng.random_range(min..=max);

        Ok(ValueHolder::Number(DynamicNumber::new(NumberHolder::Integer32(n))))
    }
});