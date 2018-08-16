// Only makes sense to use COM on Windows.
#![cfg(windows)]
// Need this for the IUnknown implementation.
#![feature(integer_atomics)]

// Interface imports.
use winapi::shared::dxgi::{IDXGIFactory, IDXGIFactoryVtbl, IDXGIObject, IDXGIObjectVtbl};
use winapi::um::unknwnbase::{IUnknown, IUnknownVtbl};

use com_impl::{implementation, interface, ComInterface};

use std::sync::atomic::{AtomicU32, Ordering};
use winapi::shared::guiddef::{IsEqualGUID, GUID};
use winapi::um::winnt::HRESULT;

#[interface(IDXGIFactory)]
pub struct FakeFactory {
    refs: AtomicU32,
    variable: u64,
}

impl FakeFactory {
    pub fn new() -> *mut IDXGIFactory {
        let fact = Self {
            __vtable: Box::new(Self::create_vtable()),
            refs: AtomicU32::new(1),
            variable: 12345,
        };

        let ptr = Box::into_raw(Box::new(fact));

        ptr as *mut _
    }
}

#[implementation(IUnknown)]
impl FakeFactory {
    fn query_interface(&mut self, riid: &GUID, obj: &mut usize) -> HRESULT {
        use winapi::shared::winerror::{E_NOTIMPL, S_OK};
        use winapi::Interface;

        *obj = 0;

        if IsEqualGUID(riid, &IDXGIFactory::uuidof())
            || IsEqualGUID(riid, &IDXGIObject::uuidof())
            || IsEqualGUID(riid, &IUnknown::uuidof())
        {
            *obj = self as *mut _ as usize;
            self.add_ref();
            S_OK
        } else {
            E_NOTIMPL
        }
    }

    fn add_ref(&mut self) -> u32 {
        let prev = self.refs.fetch_add(1, Ordering::SeqCst);
        prev + 1
    }

    fn release(&mut self) -> u32 {
        let prev = self.refs.fetch_sub(1, Ordering::SeqCst);
        if prev == 1 {
            let _box = unsafe { Box::from_raw(self as *mut _) };
        }
        prev - 1
    }
}

#[implementation(IDXGIObject)]
impl FakeFactory {
    fn set_private_data() {}
    fn set_private_data_interface() {}
    fn get_private_data() {}
    fn get_parent() {}
}

#[implementation(IDXGIFactory)]
impl FakeFactory {
    fn create_software_adapter() {}

    fn enum_adapters() {}

    fn make_window_association() {}
    fn get_window_association() {}

    fn create_swap_chain() {}
}
