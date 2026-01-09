# thin-cell

[![MIT licensed](https://img.shields.io/badge/license-MIT-blue.svg)](https://github.com/compio-rs/thin-cell/blob/master/LICENSE)
[![crates.io](https://img.shields.io/crates/v/thin-cell)](https://crates.io/crates/thin-cell)
[![docs.rs](https://img.shields.io/badge/docs.rs-thin--cell-latest)](https://docs.rs/thin-cell)
[![Check](https://github.com/compio-rs/thin-cell/actions/workflows/ci_check.yml/badge.svg)](https://github.com/compio-rs/thin-cell/actions/workflows/ci_check.yml)
[![Test](https://github.com/compio-rs/thin-cell/actions/workflows/ci_test.yml/badge.svg)](https://github.com/compio-rs/thin-cell/actions/workflows/ci_test.yml)
[![Telegram](https://img.shields.io/badge/Telegram-compio--rs-blue?logo=telegram)](https://t.me/compio_rs)


A compact, single-threaded smart pointer combining reference counting and interior mutability.

`ThinCell` is a space-efficient alternative to `Rc` and `borrow_mut`-only `RefCell` that the itself is always **1 pointer-sized** no matter if `T` is `Sized` or not (like `ThinBox`), compare to `Rc<RefCell<T>>` which is 2 pointer-sized for `T: !Sized`.

## Features

- One-`usize` pointer, no matter what `T` is
- Reference counted ownership (like `Rc`)
- Interior mutability with only mutable borrows (so it only needs 1-bit to
  track borrow state)

## How It Works

`ThinCell` achieves its compact representation by storing metadata inline at offset 0 of the allocation (for unsized types) like `ThinBox` does.

Overall layout:

```ignore
struct Inner<T> {
    metadata: usize,
    state: usize
    data: T,
}
```

## Borrow Rules

Unlike `RefCell` which supports multiple immutable borrows OR one mutable borrow, `ThinCell` only supports **one mutable borrow at a time**. Attempting to borrow while already borrowed will panic with `borrow` or return `None` with `try_borrow`.

# Examples

## Basic Usage

```rust
# use thin_cell::ThinCell;
let cell = ThinCell::new(42);

// Clone to create multiple owners
let cell2 = cell.clone();

// Borrow mutably
{
    let mut borrowed = cell.borrow();
    *borrowed = 100;
} // borrow is released here

// Access from another owner
assert_eq!(*cell2.borrow(), 100);
```

## With Trait Objects (Unsized Types)

Due to limitation of stable rust, or in particular, the lack of [`CoerceUnsized`](https://doc.rust-lang.org/std/ops/trait.CoerceUnsized.html), creating a `ThinCell<dyn Trait>` from a concrete type requires manual [`coercion`](https://doc.rust-lang.org/reference/type-coercions.html#unsized-coercions), and that coercion's safety has to be guaranteed by the user. Normally just `ptr as *const Inner<MyUnsizedType>` or `ptr as _` with external type annotation is good enough:

```rust
# use thin_cell::ThinCell;
trait Animal {
    fn speak(&self) -> &str;
}

struct Dog;

impl Animal for Dog {
    fn speak(&self) -> &str {
        "Woof!"
    }
}

// Create a ThinCell<dyn Animal> from a concrete type
// Or you can write `unsafe { ThinCell::new_unsize(Dog, |p| p as *const Inner<dyn Animal>) };`
let cell: ThinCell<dyn Animal> = unsafe { ThinCell::new_unsize(Dog, |p| p as _) };

// Still only 1 word of storage!
assert_eq!(std::mem::size_of_val(&cell), std::mem::size_of::<usize>());
```

## Borrow Checking

```rust,should_panic
use thin_cell::ThinCell;

let cell = ThinCell::new(42);

let borrow1 = cell.borrow();
let borrow2 = cell.borrow(); // Panics! Already borrowed
```

Use `try_borrow` for non-panicking behavior:

```rust
# use thin_cell::ThinCell;
let cell = ThinCell::new(42);

let borrow1 = cell.borrow();
assert!(cell.try_borrow().is_none()); // Returns None instead of panicking
```
