# Implement COM interfaces in Rust

This crate provides a procedural macro which helps with implementing
[COM interfaces](https://en.wikipedia.org/wiki/Component_Object_Model) in Rust.

**Note**: if you only want to use COM in Rust,
you can simply use [winapi](https://github.com/retep998/winapi-rs).
This crate is for implementing an interface's methods from within Rust.

**Requires Rust nightly, for now!**

## Usage

The following example shows how this crate can be used.

### At the crate level

You must add `winapi` as a dependency to your crate, and at the very least enable the `winerror` and `unknwnbase` features.

```toml
[dependencies.winapi]
version = "0.3"
features = ["winerror", "unknwnbase"]
```

You also have to enable the `integer_atomics` feature.

```rust
#![feature(integer_atomics)]
```

This is required by the `IUnknown` reference counting implementation.

### For every interface you want to implement

You must manually import the interfaces you are implementing and their vtables.

```rust
use winapi::shared::dxgi::{IDXGIObject, IDXGIObjectVtbl};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use some::other::{Interface, InterfaceVtbl};
```

Then you can import the procedural macros exported by this crate.

```rust
use com_impl::{interface, implementation};
```

Define your structure.
You must specify, in the correct order, all of the interfaces in the inheritance chain.

```rust
#[interface(IUnknown, IDXGIObject, IDXGIFactory)]
struct MyInterface {}
```

For each interface in the inheritance chain, you must have a new `implementation`.

```rust
// IUnknown's methods & reference counting are implemented automatically.

// First parameter is the base class, in this case `IUnknown`.
// Second parameter is the class you are implementing, in this case `IDXGIObject`.
#[implementation(IUnknown, IDXGIObject)]
impl MyInterface {
    // COM functions follow the PascalCase calling convention.
    // You implement a PascalCase function by using the snake_case name.

    // For example, this one implements `GetParent`.
    // Note: the macro automatically adds `unsafe extern "system"` to the function definition.
    fn get_parent(&mut self, riid: REFIID, parent: *mut c_void) -> HRESULT { /* ... */ }

    // ... Implement the other methods here ...
}
```

If we had specified `NextInterface` instead of `IDXGIObject` when defining the struct, we could continue the implementation chain here.

```rust
/// `IDXGIObject` implemented above is the parent, `NextInterface` is implemented here.
#[implementation(IDXGIObject, NextInterface)]
impl MyInterface {
    // ... New functions added by NextInterface ...
}
```

To implement the constructor for your type, use the generated `Self::create_vtable` and `Self::create_refs` functions
to fill in the generated `__vtable` and `__refs` fields.

```rust
impl MyInterface {
    // This is an example constructor.
    fn new() -> Self {
        Self {
            __vtable: Self::create_vtable(),
            __refs: Self::create_refs(),
            /* other fields */
        }
    }
}
```

Check out the `tests` directory for more examples.

## Issues

- Even if your struct is empty, you must still declare it with brackets: `struct Something { }`

- Structs with unnamed fields (e.g. `struct Example(u32, i32);`) are not supported.

- A struct can only implement one interface hierarchy.
  You cannot have a single object implementing multiple disjoint interfaces.

## License

This code is licensed under the [Mozilla Public License version 2.0](https://www.mozilla.org/en-US/MPL/2.0/).
