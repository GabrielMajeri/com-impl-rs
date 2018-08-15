pub use com_impl_macro::*;

/// Trait implemented by an interface implementation.
pub trait ComInterface<V> {
    fn create_vtable() -> V;
}
