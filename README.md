# Implement COM interfaces in Rust

This crate provides a procedural macro which helps with implementing
[COM interfaces](https://en.wikipedia.org/wiki/Component_Object_Model) in Rust.

It is currently able to implement interfaces with single-inheritance
COM hierarchies, and handles constructing the vtable automatically.

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

### For every interface you want to implement

You must manually import the **interfaces** you are implementing and **their vtables**.

```rust
use winapi::shared::dxgi::{IDXGIObject, IDXGIObjectVtbl};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};
use some::other::{Interface, InterfaceVtbl};
```

It is **very important** for the vtable to be correctly defined, otherwise external code
using your interface could misbehave. Use the `RIDL!` macro from `winapi` for maximum
compatibility.

Then you need to import the procedural macros and the `ComInterface` trait exported by this crate.

```rust
use com_impl::{ComInterface, interface, implementation};
```

Define your structure.
You must specifiy the final interface you want to implement.

```rust
#[interface(IDXGIFactory)]
struct MyInterface {}
```

For each interface in the inheritance chain, you must have a new `implementation`.

```rust
// The custom attribute's parameter is the interface you are implementing.
// In this case `IUnknown`.
#[implementation(IUnknown)]
impl MyInterface {
    // COM functions follow the PascalCase calling convention.
    // You implement a PascalCase function by using the snake_case name.

    // For example, this one implements `QueryInterface`.
    // Note: the macro automatically adds `unsafe extern "system"` to the function definition.
    fn query_interface(&self) -> HRESULT { /* ... */ }
    fn add_ref(&mut self) -> ULONG { /* ... */ }
    fn release(&mut self) -> ULONG { /* ... */ }
}

// Now we implement IDXGIObject.
#[implementation(IDXGIObject)]
impl MyInterface {
    fn get_parent(&mut self, riid: REFIID, parent: *mut c_void) -> HRESULT { /* ... */ }

    // ... Implement the other methods here ...
}
```

If we had specified `NextInterface` instead of `IDXGIObject` when defining the struct, we could continue the implementation chain here.

```rust
/// `NextInterface` is implemented here.
#[implementation(NextInterface)]
impl MyInterface {
    // ... New functions added by NextInterface ...
}
```

To implement the constructor for your type, use the generated `Self::create_vtable` function
to fill in the generated `__vtable` field.

```rust
impl MyInterface {
    // This is an example constructor.
    fn new() -> Self {
        Self {
            __vtable: Box::new(Self::create_vtable()),
            /* other fields */
        }
    }
}
```

Check out the `tests` directory for more examples.

## FAQ

- Q: Does it auto-implement `IUnknown`'s methods?

  A: No. See the test directory for an example on how to do this manually.
  Also see the [various blog posts](https://blogs.msdn.microsoft.com/oldnewthing/20040326-00/?p=40033)
  giving practical advice on implementing COM stuff.

- Q: Is it safe?

  A: Not very safe. Rust proc macros are limited in their access to the type system.
  We don't exactly have a full reflection system, so you **must** make sure your method
  implementation match the method signatures.

- Q: How to handle memory management?

  A: You have to implement an internal atomic reference counter yourself,
  and then remember to allocate any COM objects on the heap, to be able to
  easily share references to them.

  See the test code for an idea on how to use `Box`.

## Issues

- Even if your struct is empty, you must still declare it with brackets: `struct Something { }`

- Structs with unnamed fields (e.g. `struct Example(u32, i32);`) are not supported.

- A struct can only implement one interface hierarchy.
  You cannot have a single object implementing multiple disjoint interfaces.

## License

This code is licensed under the [Mozilla Public License version 2.0](https://www.mozilla.org/en-US/MPL/2.0/).
