
use nlf_macros::module;
use rand::Rng;

use crate::{argument, errors::LanguageError, interpreter::{ModuleContext, RuntimeError, RuntimeResult}, parser::ValueHolder};

module!("random", {
    fn rand(_values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let mut rng = rand::rng();
        let n = rng.random();

        Ok(ValueHolder::Number(n))
    }

    fn randint(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
        let min = argument!(values, ValueHolder::Number, "min", 0) as i64;
        let max = argument!(values, ValueHolder::Number, "max", 1) as i64;

        let mut rng = rand::rng();
        let n = rng.random_range(min..=max);

        Ok(ValueHolder::Number(n as f64))
    }
});