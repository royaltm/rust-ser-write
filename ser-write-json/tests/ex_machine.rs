// fun with enums and serde
#![cfg(any(feature = "std", feature = "alloc"))]
use core::fmt::{self, Display, Formatter};

use serde::{Serialize, Deserialize};
use serde_json::json;

use ser_write_json::{to_string, from_mut_slice};

type Vars<'a> = std::collections::HashMap<&'a str, f64>;

/// A machine
#[derive(Debug, Serialize, Deserialize, PartialEq)]
#[serde(untagged)]
enum Thing<'a> {
    /// A constant
    Num(f64),
    /// A variable name
    Var(&'a str),
    #[serde(borrow)]
    /// Some operation
    Op(Box<Op<'a>>)
}

/// Machine ops
#[derive(Debug, Serialize, Deserialize, PartialEq)]
enum Op<'a> {
    #[serde(borrow, rename="+")]
    Add(Vec<Thing<'a>>),
    #[serde(borrow, rename="-")]
    Sub(Vec<Thing<'a>>),
    #[serde(borrow, rename="*")]
    Mul(Vec<Thing<'a>>),
    #[serde(borrow, rename="/")]
    Div(Vec<Thing<'a>>),
    #[serde(rename="^")]
    Pow(Thing<'a>, Thing<'a>),
    #[serde(rename="ln")]
    Log(Thing<'a>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpKind {
    Add, Sub, Mul, Div
}

impl OpKind {
    fn reduce_head(self) -> impl Fn(f64, f64) -> f64 {
        match self {
            OpKind::Add => |x,y| x+y,
            OpKind::Sub => |x,y| x-y,
            OpKind::Mul => |x,y| x*y,
            OpKind::Div => |x,y| x/y,
        }
    }

    fn reduce_tail(self) -> impl Fn(f64, f64) -> f64 {
        match self {
            OpKind::Add|OpKind::Sub => |x,y| x+y,
            OpKind::Mul|OpKind::Div => |x,y| x*y,
        }
    }

    fn wrap<'a>(self, args: Vec<Thing<'a>>) -> Op<'a> {
        match self {
            OpKind::Add => Op::Add(args),
            OpKind::Sub => Op::Sub(args),
            OpKind::Mul => Op::Mul(args),
            OpKind::Div => Op::Div(args),
        }
    }
}

impl Display for Thing<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        match self {
            Thing::Num(x) => write!(f, "{}", x),
            Thing::Var(s) => write!(f, "{}", s),
            Thing::Op(op) => op.fmt(f)
        }
    }
}


impl Display for Op<'_> {
    fn fmt(&self, f: &mut Formatter<'_>) -> Result<(), fmt::Error> {
        let (op, args) = match self {
            Op::Add(args) => ('+', args),
            Op::Sub(args) => ('-', args),
            Op::Mul(args) => ('*', args),
            Op::Div(args) => ('/', args),
            Op::Pow(x, y) => return write!(f, "{} ^ {}", x, y),
            Op::Log(x) => return write!(f, "ln({})", x),
        };
        let mut it = args.iter();
        f.write_str("(")?;
        if let Some(t) = it.next() {
            write!(f, "{}", t)?;
        }
        for t in it {
            write!(f, " {} {}", op, t)?;
        }
        f.write_str(")")
    }
}

impl Thing<'_> {
    fn op_const(&mut self, op: impl FnOnce(f64) -> f64) -> bool {
        match self {
            Thing::Num(x) => {*x = op(*x); true}
            _ => false
        }
    }
    /// Run the machine with given vars
    fn run(&self, vars: &Vars) -> f64 {
        match self {
            Thing::Var(name) => vars[name],
            Thing::Num(v) => *v,
            Thing::Op(op) => op.run(vars)
        }
    }
    /// Reduce the machine with const vars
    fn reduce(self, consts: &Vars) -> Self {
        match self {
            Thing::Var(name) => consts.get(name).copied()
                                .map(Thing::Num)
                                .unwrap_or(self),
            Thing::Op(op) => op.reduce(consts),
            _ => self
        }
    }
}

impl<'a> Op<'a> {
    fn run(&self, input: &Vars) -> f64 {
        match self {
            Op::Add(args) => {
                args.iter().map(|x| x.run(input)).sum::<f64>()
            }
            Op::Sub(args) => {
                args[0].run(input) -
                args[1..].iter().map(|x| x.run(input)).sum::<f64>()
            }
            Op::Mul(args) => {
                args.iter().map(|x| x.run(input)).product::<f64>()
            }
            Op::Div(args) => {
                args[0].run(input) /
                args[1..].iter().map(|x| x.run(input)).product::<f64>()
            }
            Op::Pow(x, y) => x.run(input).powf(y.run(input)),
            Op::Log(x) => x.run(input).ln(),
        }
    }
    fn reduce(self, consts: &Vars) -> Thing<'a> {
        match self {
            Op::Add(args) => reduce_with(consts, args, OpKind::Add),
            Op::Sub(args) => reduce_with(consts, args, OpKind::Sub),
            Op::Mul(args) => reduce_with(consts, args, OpKind::Mul),
            Op::Div(args) => reduce_with(consts, args, OpKind::Div),
            // the following 2 could do better
            Op::Pow(x, y) => match (x.reduce(consts), y.reduce(consts)) {
                (Thing::Num(x), Thing::Num(y)) => Thing::Num(x.powf(y)),
                (x, y) => Thing::Op(Box::new(Op::Pow(x, y)))
            }
            Op::Log(x) => {
                let mut x = x.reduce(consts);
                if x.op_const(|c| c.ln()) {
                    x
                }
                else {
                    Thing::Op(Box::new(Op::Log(x)))
                }
            }
        }
    }
    fn fold_tail(&mut self, kind: OpKind, cc: f64) -> bool {
        match (self, kind) {
            // ((x + c) + cc) -> (x + (c + cc))
            (Op::Add(args), OpKind::Add)|
            // ((x - c) - cc) -> (x - (c + cc))
            (Op::Sub(args), OpKind::Sub) =>  {
                args.len() == 2 && args[1].op_const(|c| c+cc)
            }
            // ((x + c) - cc) -> (x + (c - cc))
            (Op::Add(args), OpKind::Sub)|
            // ((x - c) + cc) -> (x - (c - cc))
            (Op::Sub(args), OpKind::Add) =>  {
                args.len() == 2 && args[1].op_const(|c| c-cc)
            }
            // ((x * c) * cc) -> (x * (c * cc))
            (Op::Mul(args), OpKind::Mul)|
            // ((x / c) / cc) -> (x / (c * cc))
            (Op::Div(args), OpKind::Div) =>  {
                args.len() == 2 && args[1].op_const(|c| c*cc)
            }
            // ((x * c) / cc) -> (x * (c / cc))
            (Op::Mul(args), OpKind::Div)|
            // ((x / c) * cc) -> (x / (c / cc))
            (Op::Div(args), OpKind::Mul) => {
                args.len() == 2 && args[1].op_const(|c| c/cc)
            }
            _ => false
        }
    }
}

/// Reduce arguments to a Thing::Op given operations
fn reduce_with<'a>(
        consts: &Vars,
        args: Vec<Thing<'a>>,
        kind: OpKind
    ) -> Thing<'a>
{
    let mut res = Vec::with_capacity(args.len());
    let mut iter = args.into_iter();
    let head = iter.next().expect("missing 1st argument").reduce(consts);

    res.push(head);

    let reduce_tail = kind.reduce_tail();
    let reduce_head = kind.reduce_head();
    let acc = iter.fold(None, |acc, thing| {
        match thing.reduce(consts) {
            Thing::Num(y) => acc.map(|x| reduce_tail(x, y)).or(Some(y)),
            t => {
                res.push(t);
                acc
            }
        }
    });
    if res.len() == 1 {
        let acc = acc.expect("missing 2nd argument");
        match res.first_mut().unwrap() {
            Thing::Num(x) => return Thing::Num(reduce_head(*x, acc)),
            Thing::Op(op) => {
                if op.fold_tail(kind, acc) {
                    return res.into_iter().next().unwrap()
                }
            }
            _ => {}
        }
        res.push(Thing::Num(acc));
        Thing::Op(Box::new(kind.wrap(res)))
    }
    else {
        match res.first().unwrap() {
            &Thing::Num(x) => {
                res[0] = Thing::Num(
                    acc.map(|y| reduce_head(x, y)
                ).unwrap_or(x));
            }
            _ => if let Some(y) = acc {
                res.push(Thing::Num(y));
            }
        }
        Thing::Op(Box::new(kind.wrap(res)))
    }
}

#[test]
fn test_thing_simple() {
    let func = json!({
        "+": [2, 2]
    });
    let s = to_string(&func).unwrap();
    assert_eq!(s,
        r#"{"+":[2,2]}"#
    );
    let mut vec = s.into_bytes();
    let thing: Thing = from_mut_slice(&mut vec).unwrap();
    // println!("{}", thing);
    assert_eq!(format!("{}", thing), "(2 + 2)");

    let vars = Vars::new();
    let res = thing.run(&vars);
    // println!("{} = {}", thing, res);
    assert_eq!(res, 4.0);

    let thing = thing.reduce(&vars);
    // println!("{}", thing);
    assert_eq!(format!("{}", thing), "4");
    let s = to_string(&thing).unwrap();
    assert_eq!(s,"4");

    let res = thing.run(&vars);
    // println!("{} = {}", thing, res);
    assert_eq!(res, 4.0);
}

#[test]
fn test_thing_temperature() {
    //  (Temperature in degrees Celsius (°C) * 9/5) + 32
    let func = json!({
        "+": [{
            "*": ["ctemp",
                  {"/": [9, 5]}]
        }, 32] // +
    });
    let s = to_string(&func).unwrap();
    assert_eq!(s,
        r#"{"+":[{"*":["ctemp",{"/":[9,5]}]},32]}"#
    );
    let mut vec = s.into_bytes();
    let thing: Thing = from_mut_slice(&mut vec).unwrap();
    // println!("{}", thing);
    assert_eq!(format!("{}", thing), "((ctemp * (9 / 5)) + 32)");

    let mut vars = Vars::new();
    vars.insert("ctemp", 36.6);

    let res = thing.run(&vars);
    // println!("temp: {}°C = {}°F", vars["ctemp"], res);
    assert_eq!(res, 36.6*1.8+32.0);

    vars.clear();
    let thing = thing.reduce(&vars);
    // println!("{}", thing);
    assert_eq!(format!("{}", thing), "((ctemp * 1.8) + 32)");
    let s = to_string(&thing).unwrap();
    assert_eq!(s,
        r#"{"+":[{"*":["ctemp",1.8]},32]}"#
    );

    vars.insert("ctemp", 36.6);
    let res = thing.run(&vars);
    // println!("temp: {}°C = {}°F", vars["ctemp"], res);
    assert_eq!(res, 36.6*1.8+32.0);
}

#[test]
fn test_thing_cosmic_v1() {
    const G : f64 = 6.6743015e-11; // N*m^2/kg^2
    const MZ: f64 = 5.9722e24; // kg
    const R : f64 = 6378140.0; // m

    //  v = (G*Mz/r)^1/2
    let cosmos1 = json!({
        "^": [{
            "/": [{
                "*": ["G", "Mz"]},
                "r"]
        },
        {"/": [1, 2]}] // ^
    });
    let s = to_string(&cosmos1).unwrap();
    assert_eq!(s,
        r#"{"^":[{"/":[{"*":["G","Mz"]},"r"]},{"/":[1,2]}]}"#
    );
    let mut vec = s.into_bytes();
    let thing: Thing = from_mut_slice(&mut vec).unwrap();

    // thing*3600/1000
    let to_kmh = json!({
        "/": [
            {"*": [thing, 3600]},
            1000
        ]
    });
    let s = to_string(&to_kmh).unwrap();
    assert_eq!(s,
        r#"{"/":[{"*":[{"^":[{"/":[{"*":["G","Mz"]},"r"]},{"/":[1,2]}]},3600]},1000]}"#
    );
    let mut vec = s.into_bytes();
    let thing: Thing = from_mut_slice(&mut vec).unwrap();
    // println!("{}", thing);
    assert_eq!(format!("{}", thing), "((((G * Mz) / r) ^ (1 / 2) * 3600) / 1000)");

    let expect: f64 = 28459.388161308587; // km/h

    let mut vars = Vars::new();
    vars.insert("G", G);
    vars.insert("Mz", MZ);
    vars.insert("r", R);

    let res = thing.run(&vars);
    // println!("v1: {} km/h", res);
    assert!((res - expect).abs() < f64::EPSILON);

    vars.remove("r");
    let thing = thing.reduce(&vars);
    // println!("{}", thing);
    assert_eq!(format!("{}", thing), "((398602634183000 / r) ^ 0.5 * 3.6)");

    let s = to_string(&thing).unwrap();
    assert_eq!(s,
        r#"{"*":[{"^":[{"/":[398602634183000,"r"]},0.5]},3.6]}"#
    );

    vars.insert("r", R);
    let res = thing.run(&vars);
    // println!("v1: {} km/h", res);
    assert!((res - expect).abs() < f64::EPSILON);
}

#[test]
fn test_thing_rtd() {
    let r_rtd = json!({
        "*": [
            6810,
            {"/": [
                "adc-value",
                {"-": [2097152, "adc-value"]}
            ]}
        ]
    });
    let log_r_rtd = json!({
        "ln": r_rtd
    });
    let v_celsius = json!({
        "-": [
            {"/": [
                1,
                {"+": [
                    0.001403,
                    {"*": [0.0002373, log_r_rtd]},
                    {"*": [0.00000009827, {"^": [log_r_rtd, 3]}]}
                ]}
            ]},
            273.25
        ]
    });
    let s = to_string(&v_celsius).unwrap();
    // println!("\r\n{}", s);
    let mut vec = s.into_bytes();
    let thing: Thing = from_mut_slice(&mut vec).unwrap();
    // println!("\r\n{}", thing);
    assert_eq!(
        format!("{}", thing),
        "((1 / (0.001403 + (0.0002373 * ln((6810 * (adc-value / (2097152 - adc-value))))) + (0.00000009827 * ln((6810 * (adc-value / (2097152 - adc-value)))) ^ 3))) - 273.25)"
    );
    let mut vars = Vars::new();
    vars.insert("adc-value", 374.0);
    let res = thing.run(&vars); // 
    assert!((res - 416.80808076956555).abs() < f64::EPSILON);
    // println!("ADC: {} -> {}°C", vars["adc-value"], res);
    vars.insert("adc-value", 627731.0);
    let res = thing.run(&vars); // 25.66128620883586
    // println!("ADC: {} -> {}°C", vars["adc-value"], res);
    assert!((res - 25.66128620883586).abs() < f64::EPSILON);

    let thing = thing.reduce(&vars);
    // println!("\r\n{}", thing);
    assert_eq!(format!("{}", thing), "25.66128620883586");
}
