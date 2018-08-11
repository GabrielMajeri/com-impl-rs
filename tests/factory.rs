// Only makes sense to use COM on Windows.
#![cfg(windows)]
// Need this for the IUnknown implementation.
#![feature(integer_atomics)]

// Interface imports.
use winapi::shared::dxgi::{IDXGIFactory, IDXGIFactoryVtbl, IDXGIObject, IDXGIObjectVtbl};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};

use com_impl::{implementation, interface};

#[interface(IUnknown, IDXGIObject, IDXGIFactory)]
pub struct FakeFactory {
    variable: u64,
}

impl FakeFactory {
    pub fn new() -> *mut IDXGIFactory {
        let fact = Self {
            __vtable: Self::create_vtable(),
            __refs: Self::create_refs(),
            variable: 12345,
        };

        let ptr = Box::into_raw(Box::new(fact));

        ptr as *mut _
    }
}

#[implementation(IUnknown, IDXGIObject)]
impl FakeFactory {
    fn set_private_data() {}
    fn set_private_data_interface() {}
    fn get_private_data() {}
    fn get_parent() {}
}

#[implementation(IDXGIObject, IDXGIFactory)]
impl FakeFactory {
    fn create_software_adapter() {}

    fn enum_adapters() {}

    fn make_window_association() {}
    fn get_window_association() {}

    fn create_swap_chain() {}
}
