#![feature(register_tool)]
#![register_tool(flux)]
#![feature(custom_inner_attributes)]
#![flux::dfn(nat(x: int) -> bool { leq(0, x) })]
#![flux::dfn(leq(x: int, y: int) -> bool { x <= y })]
#![flux::dfn(inc(x: int) -> int { x + 1 })]

#[flux::alias(type Nat() = i32{v: nat(v)})]
type _Nat = i32;

#[flux::alias(type Lb(n) = i32{v: leq(n, v) })]
type _Lb = i32;

#[flux::sig(fn(x:Nat) -> Nat)]
pub fn test1(x: i32) -> i32 {
    x - 1 //~ ERROR postcondition
}

#[flux::sig(fn(x:Lb[10]) -> Lb[10])]
pub fn test2(x: i32) -> i32 {
    x - 1 //~ ERROR postcondition
}

#[flux::sig(fn(x:i32) -> i32[inc(x)])]
pub fn test3(x: i32) -> i32 {
    x + 2 //~ ERROR postcondition
}