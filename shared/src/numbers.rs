use std::{cmp::Ordering, fmt, ops, str::FromStr};

use serde::Serialize;
use strum::{AsRefStr, EnumCount, EnumIter, EnumString, FromRepr};

#[derive(Debug, Serialize, EnumIter, EnumCount, FromRepr, EnumString, AsRefStr, Clone, Copy)]
#[repr(u8)]
pub enum NumberHolder {
    Unsigned8(u8),
    Unsigned16(u16),
    Unsigned32(u32),
    Unsigned64(u64),
    Unsigned128(u128),
    Integer8(i8),
    Integer16(i16),
    Integer32(i32),
    Integer64(i64),
    Integer128(i128),
    Float32(f32),
    Float64(f64)
}

impl NumberHolder {
    #[inline(always)]
    pub fn get_weight(&self) -> u8 {
        match self {
            NumberHolder::Unsigned8(_) => 0,
            NumberHolder::Unsigned16(_) => 1,
            NumberHolder::Unsigned32(_) => 2,
            NumberHolder::Unsigned64(_) => 3,
            NumberHolder::Unsigned128(_) => 4,
            NumberHolder::Integer8(_) => 5,
            NumberHolder::Integer16(_) => 6,
            NumberHolder::Integer32(_) => 7,
            NumberHolder::Integer64(_) => 8,
            NumberHolder::Integer128(_) => 9,
            NumberHolder::Float32(_) => 10,
            NumberHolder::Float64(_) => 11,
        }
    }
}

macro_rules! into {
    ($type:ident) => {
        impl Into<$type> for DynamicNumber {
            fn into(self) -> $type {
                match self.inner {
                    NumberHolder::Integer8(v) => v as $type,
                    NumberHolder::Integer16(v) => v as $type,
                    NumberHolder::Integer32(v) => v as $type,
                    NumberHolder::Integer64(v) => v as $type,
                    NumberHolder::Integer128(v) => v as $type,
                    NumberHolder::Unsigned8(v) => v as $type,
                    NumberHolder::Unsigned16(v) => v as $type,
                    NumberHolder::Unsigned32(v) => v as $type,
                    NumberHolder::Unsigned64(v) => v as $type,
                    NumberHolder::Unsigned128(v) => v as $type,
                    NumberHolder::Float32(v) => v as $type,
                    NumberHolder::Float64(v) => v as $type
                }
            }
        }
    };
}

into!(i8);
into!(i16);
into!(i32);
into!(i64);
into!(i128);
into!(isize);
into!(u8);
into!(u16);
into!(u32);
into!(u64);
into!(u128);
into!(usize);
into!(f32);
into!(f64);

macro_rules! from {
    ($type:ident, $name:ident) => {
        impl From<$type> for DynamicNumber {
            fn from(value: $type) -> Self {
                DynamicNumber::new(NumberHolder::$name(value))
            }
        }
    };
    ($type:ident, $name:ident, $as:ident) => {
        impl From<$type> for DynamicNumber {
            fn from(value: $type) -> Self {
                DynamicNumber::new(NumberHolder::$name(value as $as))
            }
        }
    };
}

from!(u8, Unsigned8);
from!(u16, Unsigned16);
from!(u32, Unsigned32);
from!(u64, Unsigned64);
from!(u128, Unsigned128);
#[cfg(target_pointer_width = "32")]
from!(usize, Unsigned32, u32);
#[cfg(target_pointer_width = "64")]
from!(usize, Unsigned64, u64);

from!(i8, Integer8);
from!(i16, Integer16);
from!(i32, Integer32);
from!(i64, Integer64);
from!(i128, Integer128);
#[cfg(target_pointer_width = "32")]
from!(isize, Integer32, i32);
#[cfg(target_pointer_width = "64")]
from!(isize, Integer64, i64);

from!(f32, Float32);
from!(f64, Float64);

macro_rules! overload_operator {
    ($name:ident, $fn_name:ident, $checked_fn_name:ident) => {
        impl ops::$name for NumberHolder {
            type Output = Option<Self>;

            fn $fn_name(self, rhs: Self) -> Self::Output {
                match (self, rhs) {
                    (NumberHolder::Integer8(a), NumberHolder::Integer8(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Integer8(value)),
                    (NumberHolder::Integer16(a), NumberHolder::Integer16(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Integer16(value)),
                    (NumberHolder::Integer32(a), NumberHolder::Integer32(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Integer32(value)),
                    (NumberHolder::Integer64(a), NumberHolder::Integer64(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Integer64(value)),
                    (NumberHolder::Integer128(a), NumberHolder::Integer128(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Integer128(value)),
                    (NumberHolder::Unsigned8(a), NumberHolder::Unsigned8(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Unsigned8(value)),
                    (NumberHolder::Unsigned16(a), NumberHolder::Unsigned16(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Unsigned16(value)),
                    (NumberHolder::Unsigned32(a), NumberHolder::Unsigned32(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Unsigned32(value)),
                    (NumberHolder::Unsigned64(a), NumberHolder::Unsigned64(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Unsigned64(value)),
                    (NumberHolder::Unsigned128(a), NumberHolder::Unsigned128(b)) => a.$checked_fn_name(b).map(|value| NumberHolder::Unsigned128(value)),
                    (NumberHolder::Float32(a), NumberHolder::Float32(b)) => Some(NumberHolder::Float32(a.$fn_name(b))),
                    (NumberHolder::Float64(a), NumberHolder::Float64(b)) => Some(NumberHolder::Float64(a.$fn_name(b))),
                    _ => panic!()
                }
            }
        }
    };
}

overload_operator!(Add, add, checked_add);
overload_operator!(Sub, sub, checked_sub);
overload_operator!(Mul, mul, checked_mul);
overload_operator!(Div, div, checked_div);
overload_operator!(Rem, rem, checked_rem);

impl PartialEq for NumberHolder {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (NumberHolder::Integer8(a), NumberHolder::Integer8(b)) => *a == *b,
            (NumberHolder::Integer16(a), NumberHolder::Integer16(b)) => *a == *b,
            (NumberHolder::Integer32(a), NumberHolder::Integer32(b)) => *a == *b,
            (NumberHolder::Integer64(a), NumberHolder::Integer64(b)) => *a == *b,
            (NumberHolder::Integer128(a), NumberHolder::Integer128(b)) => *a == *b,
            (NumberHolder::Unsigned8(a), NumberHolder::Unsigned8(b)) => *a == *b,
            (NumberHolder::Unsigned16(a), NumberHolder::Unsigned16(b)) => *a == *b,
            (NumberHolder::Unsigned32(a), NumberHolder::Unsigned32(b)) => *a == *b,
            (NumberHolder::Unsigned64(a), NumberHolder::Unsigned64(b)) => *a == *b,
            (NumberHolder::Unsigned128(a), NumberHolder::Unsigned128(b)) => *a == *b,
            (NumberHolder::Float32(a), NumberHolder::Float32(b)) => *a == *b,
            (NumberHolder::Float64(a), NumberHolder::Float64(b)) => *a == *b,
            _ => false
        }
    }
}

impl Eq for NumberHolder {}

impl PartialOrd for NumberHolder {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for NumberHolder {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self, other) {
            (NumberHolder::Integer8(a), NumberHolder::Integer8(b)) => a.cmp(b),
            (NumberHolder::Integer16(a), NumberHolder::Integer16(b)) => a.cmp(b),
            (NumberHolder::Integer32(a), NumberHolder::Integer32(b)) => a.cmp(b),
            (NumberHolder::Integer64(a), NumberHolder::Integer64(b)) => a.cmp(b),
            (NumberHolder::Integer128(a), NumberHolder::Integer128(b)) => a.cmp(b),
            (NumberHolder::Unsigned8(a), NumberHolder::Unsigned8(b)) => a.cmp(b),
            (NumberHolder::Unsigned16(a), NumberHolder::Unsigned16(b)) => a.cmp(b),
            (NumberHolder::Unsigned32(a), NumberHolder::Unsigned32(b)) => a.cmp(b),
            (NumberHolder::Unsigned64(a), NumberHolder::Unsigned64(b)) => a.cmp(b),
            (NumberHolder::Unsigned128(a), NumberHolder::Unsigned128(b)) => a.cmp(b),
            (NumberHolder::Float32(a), NumberHolder::Float32(b)) => a.total_cmp(b),
            (NumberHolder::Float64(a), NumberHolder::Float64(b)) => a.total_cmp(b),
            _ => panic!()
        }
    }
}

#[derive(Debug, Serialize, Clone, Copy)]
pub struct DynamicNumber {
    inner: NumberHolder,
}

impl DynamicNumber {
    #[inline(always)]
    pub fn new(inner: NumberHolder) -> Self {
        Self {
            inner
        }
    }

    pub fn convert_to_type_of(&self, other: &DynamicNumber) -> DynamicNumber {
        macro_rules! cast_to {
            ($v:expr) => {
                match other.inner {
                    NumberHolder::Unsigned8(_) => DynamicNumber::new(NumberHolder::Unsigned8($v as u8)),
                    NumberHolder::Unsigned16(_) => DynamicNumber::new(NumberHolder::Unsigned16($v as u16)),
                    NumberHolder::Unsigned32(_) => DynamicNumber::new(NumberHolder::Unsigned32($v as u32)),
                    NumberHolder::Unsigned64(_) => DynamicNumber::new(NumberHolder::Unsigned64($v as u64)),
                    NumberHolder::Unsigned128(_) => DynamicNumber::new(NumberHolder::Unsigned128($v as u128)),
                    NumberHolder::Integer8(_) => DynamicNumber::new(NumberHolder::Integer8($v as i8)),
                    NumberHolder::Integer16(_) => DynamicNumber::new(NumberHolder::Integer16($v as i16)),
                    NumberHolder::Integer32(_) => DynamicNumber::new(NumberHolder::Integer32($v as i32)),
                    NumberHolder::Integer64(_) => DynamicNumber::new(NumberHolder::Integer64($v as i64)),
                    NumberHolder::Integer128(_) => DynamicNumber::new(NumberHolder::Integer128($v as i128)),
                    NumberHolder::Float32(_) => DynamicNumber::new(NumberHolder::Float32($v as f32)),
                    NumberHolder::Float64(_) => DynamicNumber::new(NumberHolder::Float64($v as f64))
                }
            };
        }

        match self.inner {
            NumberHolder::Unsigned8(v) => cast_to!(v),
            NumberHolder::Unsigned16(v) => cast_to!(v),
            NumberHolder::Unsigned32(v) => cast_to!(v),
            NumberHolder::Unsigned64(v) => cast_to!(v),
            NumberHolder::Unsigned128(v) => cast_to!(v),
            NumberHolder::Integer8(v) => cast_to!(v),
            NumberHolder::Integer16(v) => cast_to!(v),
            NumberHolder::Integer32(v) => cast_to!(v),
            NumberHolder::Integer64(v) => cast_to!(v),
            NumberHolder::Integer128(v) => cast_to!(v),
            NumberHolder::Float32(v) => cast_to!(v),
            NumberHolder::Float64(v) => cast_to!(v)
        }
    }

    pub fn get_str_repr(&self) -> String {
        self.inner.as_ref().to_string()
    }
}

fn remove_numbers(s: &str) -> String {
    s.chars().filter(|c| !c.is_ascii_digit()).collect()
}

fn get_digits(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii_digit()).collect()
}

#[inline(always)]
fn align_types(a: DynamicNumber, b: DynamicNumber) -> (DynamicNumber, DynamicNumber) {
    match a.inner.get_weight().cmp(&b.inner.get_weight()) {
        Ordering::Greater => (a, b.convert_to_type_of(&a)),
        Ordering::Less    => (a.convert_to_type_of(&b), b),
        Ordering::Equal   => (a, b),
    }
}
 
impl ops::Add for DynamicNumber {
    type Output = DynamicNumber;

    fn add(self, rhs: Self) -> Self::Output {
        let (mut a, mut b) = align_types(self, rhs);

        let mut result = a.inner + b.inner;
        if result.is_none() {
            let current = a.inner.as_ref();
            
            let id = a.inner.get_weight();
            let category: String = remove_numbers(current);

            let next = NumberHolder::from_repr(id + 1).expect("Overflowed");
            let next_category = remove_numbers(next.as_ref());

            if category == next_category {
                a = a.convert_to_type_of(&DynamicNumber::new(next));
                b = b.convert_to_type_of(&DynamicNumber::new(next));
            } else {
                panic!("Overflowed")
            }

            result = a.inner + b.inner;
        }

        DynamicNumber::new(result.unwrap())
    }
}

impl ops::Sub for DynamicNumber {
    type Output = DynamicNumber;

    fn sub(self, rhs: Self) -> Self::Output {
        let (mut a, mut b) = align_types(self, rhs);

        let mut result = a.inner - b.inner;
        if result.is_none() {
            let current = a.inner.as_ref();
            let category: String = remove_numbers(current);
            let size: u32 = get_digits(current).parse().unwrap();

            if category == "Unsigned" {
                let next = NumberHolder::from_str(format!("{}{}", "Integer", size * 2).as_str())
                    .unwrap_or(NumberHolder::from_str(format!("{}{}", "Integer", size).as_str()).expect("Overflowed"));
                a = a.convert_to_type_of(&DynamicNumber::new(next));
                b = b.convert_to_type_of(&DynamicNumber::new(next));
                
                result = a.inner - b.inner;
            }
        }

        if result.is_none() {
            let current = a.inner.as_ref();
            
            let id = a.inner.get_weight();
            let category: String = remove_numbers(current);

            let next = NumberHolder::from_repr(id + 1).expect("Overflowed");
            let next_category = remove_numbers(next.as_ref());

            if category == next_category {
                a = a.convert_to_type_of(&DynamicNumber::new(next));
                b = b.convert_to_type_of(&DynamicNumber::new(next));
            } else {
                panic!("Overflowed")
            }

            result = a.inner - b.inner;
        }

        DynamicNumber::new(result.unwrap())
    }
}

impl ops::Mul for DynamicNumber {
    type Output = DynamicNumber;

    fn mul(self, rhs: Self) -> Self::Output {
        let (mut a, mut b) = align_types(self, rhs);

        let mut result = a.inner * b.inner;
        while result.is_none() {
            let current = a.inner.as_ref();
            
            let id = a.inner.get_weight();
            let category: String = remove_numbers(current);

            let next = NumberHolder::from_repr(id + 1).expect("Overflowed");
            let next_category = remove_numbers(next.as_ref());

            if category == next_category {
                a = a.convert_to_type_of(&DynamicNumber::new(next));
                b = b.convert_to_type_of(&DynamicNumber::new(next));
            } else {
                panic!("Overflowed")
            }

            result = a.inner * b.inner;
        }

        DynamicNumber::new(result.unwrap())
    }
}

impl ops::Div for DynamicNumber {
    type Output = DynamicNumber;

    fn div(self, rhs: Self) -> Self::Output {
        let (mut a, mut b) = align_types(self, rhs);

        if a.inner.as_ref() != "Float32" && a.inner.as_ref() != "Float64"  {
            let new_type = DynamicNumber::new(NumberHolder::Float32(0.0));
            b = b.convert_to_type_of(&new_type);
            a = a.convert_to_type_of(&new_type);
        }

        let mut result = a.inner / b.inner;
        while result.is_none() {
            let current = a.inner.as_ref();
            
            let id = a.inner.get_weight();
            let category: String = remove_numbers(current);

            let next = NumberHolder::from_repr(id + 1).expect("Overflowed");
            let next_category = remove_numbers(next.as_ref());

            if category == next_category {
                a = a.convert_to_type_of(&DynamicNumber::new(next));
                b = b.convert_to_type_of(&DynamicNumber::new(next));
            } else {
                panic!("Overflowed")
            }

            result = a.inner / b.inner;
        }

        DynamicNumber::new(result.unwrap())
    }
}

impl ops::Rem for DynamicNumber {
    type Output = DynamicNumber;

    fn rem(self, rhs: Self) -> Self::Output {
        let (a, b) = align_types(self, rhs);

        let result = a.inner % b.inner;

        DynamicNumber::new(result.unwrap())
    }
}

impl fmt::Display for DynamicNumber {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self.inner {
            NumberHolder::Unsigned8(v) => format!("{}", v),
            NumberHolder::Unsigned16(v) => format!("{}", v),
            NumberHolder::Unsigned32(v) => format!("{}", v),
            NumberHolder::Unsigned64(v) => format!("{}", v),
            NumberHolder::Unsigned128(v) => format!("{}", v),
            NumberHolder::Integer8(v) => format!("{}", v),
            NumberHolder::Integer16(v) => format!("{}", v),
            NumberHolder::Integer32(v) => format!("{}", v),
            NumberHolder::Integer64(v) => format!("{}", v),
            NumberHolder::Integer128(v) => format!("{}", v),
            NumberHolder::Float32(v) => format!("{}", v),
            NumberHolder::Float64(v) => format!("{}", v),
        };

        write!(f, "{}", value)
    }
}

impl PartialEq for DynamicNumber {
    fn eq(&self, other: &Self) -> bool {
        let mut a = *self;
        let mut b = *other;

        if a.inner.get_weight() > b.inner.get_weight() {
            b = other.convert_to_type_of(&a);
        } else if b.inner.get_weight() > a.inner.get_weight() {
            a = self.convert_to_type_of(&b);
        }

        a.inner == b.inner
    }
}

impl Eq for DynamicNumber {}

impl PartialOrd for DynamicNumber {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for DynamicNumber {
    fn cmp(&self, other: &Self) -> Ordering {
        let mut a = *self;
        let mut b = *other;

        if a.inner.get_weight() > b.inner.get_weight() {
            b = other.convert_to_type_of(&a);
        } else if b.inner.get_weight() > a.inner.get_weight() {
            a = self.convert_to_type_of(&b);
        }

        a.inner.cmp(&b.inner)
    }
}

macro_rules! try_parse {
    ($str:expr, $value:expr, $type:ident, $holder_type:ident) => {
        if $value.is_none() {
            let result: Result<$type, _> = $str.parse();
            $value = result.ok().map(|value| NumberHolder::$holder_type(value));
        }
    };
}

impl DynamicNumber {
    pub fn from_str(str: &str) -> Self {
        if str == "" {
            panic!("No number provided")
        }

        let mut value: Option<NumberHolder> = None;

        let has_decimal = str.split(".").count() == 2;

        if has_decimal {
            try_parse!(str, value, f32, Float32);
            try_parse!(str, value, f64, Float64);
        } else if str.starts_with("-") {
            try_parse!(str, value, i8, Integer8);
            try_parse!(str, value, i16, Integer16);
            try_parse!(str, value, i32, Integer32);
            try_parse!(str, value, i64, Integer64);
            try_parse!(str, value, i128, Integer128);
        } else  {
            try_parse!(str, value, u8, Unsigned8);
            try_parse!(str, value, u16, Unsigned16);
            try_parse!(str, value, u32, Unsigned32);
            try_parse!(str, value, u64, Unsigned64);
            try_parse!(str, value, u128, Unsigned128);
        }

        let value = value.expect("Failed to parse number");

        DynamicNumber::new(value)
    }
}