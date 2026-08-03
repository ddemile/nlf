
use nlf_macros::module;
use nlf_shared::{errors::LanguageResult, vm::{VMContext, Value}};
use rand::Rng;

module!("random", {
    fn rand(_: VMContext) -> LanguageResult<Value> {
        let mut rng = rand::rng();
        let n = rng.random();

        Ok(Value::Float(n))
    } 

    fn randint(min: f64, max: f64, _: VMContext) -> LanguageResult<Value> {
        let mut rng = rand::rng();
        let n = rng.random_range((min as i64)..=(max as i64));

        Ok(Value::Float(n as f64))
    } 

    // fn randint(values: &[ValueHolder], _context: &mut ModuleContext) -> RuntimeResult {
    //     let min = argument!(values, ValueHolder::Number, "min", 0) as i64;
    //     let max = argument!(values, ValueHolder::Number, "max", 1) as i64;

    //     let mut rng = rand::rng();
    //     let n = rng.random_range(min..=max);

    //     Ok(ValueHolder::Number(n as f64))
    // }
});