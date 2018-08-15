#![recursion_limit = "128"]

#[macro_use]
extern crate quote;

#[macro_use]
extern crate syn;

use heck::CamelCase;

use proc_macro::TokenStream;

use syn::punctuated::Punctuated;
use syn::synom::Synom;
use syn::fold::Fold;
use syn::{Ident, ItemStruct, Fields, FieldsNamed};

#[proc_macro_attribute]
pub fn interface(attr: TokenStream, input: TokenStream) -> TokenStream {
    let Args { parents } =
        syn::parse(attr).expect("You must the interface to implement");

    assert_eq!(parents.len(), 1, "You can only implement one interface");

    // Return the original input if it fails to parse.
    let input: syn::ItemStruct = match syn::parse(input.clone()) {
        Ok(input) => input,
        Err(_) => return input,
    };

    let struct_name = &input.ident.clone();

    let vtables: Vec<_> = parents.iter()
        .map(|iface| VTable { iface })
        .collect();

    let last_vtable = vtables.last().unwrap();


    let mut iunknown = IUnknownImpl { vtable: &last_vtable };
    let input = iunknown.fold_item_struct(input);


    let expanded = quote! {
        #input

        impl #struct_name {
            fn create_refs() -> std::sync::atomic::AtomicU32 {
                std::sync::atomic::AtomicU32::new(1)
            }

            extern "system" fn query_interface(&mut self, riid: &winapi::shared::guiddef::GUID, obj: &mut usize) -> winapi::um::winnt::HRESULT {
                use winapi::Interface;
                use winapi::shared::winerror::{S_OK, E_NOTIMPL};

                *obj = 0;

                #(
                    if unsafe { winapi::shared::guiddef::IsEqualGUID(riid, &#parents::uuidof()) } {
                        *obj = self as *mut _ as usize;
                        self.add_ref();
                        return S_OK;
                    }
                )*

                return E_NOTIMPL;
            }

            extern "system" fn add_ref(&mut self) -> u32 {
                let prev = self.__refs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                prev + 1
            }

            extern "system" fn release(&mut self) -> u32 {
                let prev = self.__refs.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if prev == 1 {
                    let _box = unsafe { Box::from_raw(self as *mut _) };
                }
                prev - 1
            }
        }

        impl com_impl::ComInterface<IUnknownVtbl> for #struct_name {
            fn create_vtable() -> IUnknownVtbl {
                unsafe {
                    IUnknownVtbl {
                        QueryInterface: std::mem::transmute(Self::query_interface as usize),
                        AddRef: std::mem::transmute(Self::add_ref as usize),
                        Release: std::mem::transmute(Self::release as usize),
                    }
                }
            }
        }
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn implementation(attr: TokenStream, input: TokenStream) -> TokenStream {
    let Args { parents } =
        syn::parse(attr).expect("Failed to parse attribute arguments");

    assert_eq!(parents.len(), 2, "Expected two interfaces: the parent and the implemented interface");

    let input: syn::ItemImpl = syn::parse(input).expect("Could not parse interface impl block");

    let mut imp = Implementation { fns: Vec::new() };

    let input = imp.fold_item_impl(input);

    let self_ty = &input.self_ty;

    let vtable = VTable { iface: &parents[1] }.ident();

    let fns = &imp.fns;
    let methods = fns.iter().map(|id| {
        let method_name = id.to_string().to_camel_case();
        syn::Ident::new(&method_name, id.span())
    });

    let vtable_creator = quote! {
        impl com_impl::ComInterface<#vtable> for #self_ty {
            fn create_vtable() -> #vtable {
                use com_impl::ComInterface;
                unsafe {
                    #vtable {
                        parent: Self::create_vtable(),
                        #(#methods: std::mem::transmute((Self::#fns) as usize),)*
                    }
                }
            }
        }
    };

    let expanded = quote! {
        #input
        #vtable_creator
    };

    expanded.into()
}

struct Args {
    parents: Vec<Ident>,
}

impl Synom for Args {
    named!(parse -> Self, map!(
        call!(Punctuated::<Ident, Token![,]>::parse_terminated_nonempty),
        |parents| Args {
            parents: parents.into_iter().collect(),
        }
    ));
}

struct VTable<'a> {
    iface: &'a Ident,
}

impl<'a> VTable<'a> {
    fn ident(&self) -> Ident {
        let vtable_name = format!("{}Vtbl", self.iface);
        Ident::new(&vtable_name, self.iface.span())
    }
}

struct IUnknownImpl<'a> {
    // The identifier of the last-level VTable.
    vtable: &'a VTable<'a>,
}

impl<'a> Fold for IUnknownImpl<'a> {
    fn fold_item_struct(&mut self, mut st: ItemStruct) -> ItemStruct {
        // Ensure the layout of the struct is fixed.
        st.attrs.push(parse_quote!(#[repr(C)]));

        st.fields = self.fold_fields(st.fields);

        st
    }

    fn fold_fields(&mut self, f: Fields) -> Fields {
        match f {
            Fields::Named(named) => Fields::Named(self.fold_fields_named(named)),
            Fields::Unnamed(_) => panic!("Only structs with named fields are supported"),
            Fields::Unit => panic!("Unit structs not supported, please append `{ }` to the struct definition"),
        }
    }

    fn fold_fields_named(&mut self, f: FieldsNamed) -> FieldsNamed {
        let vtable = self.vtable.ident();
        let named = f.named;
        parse_quote! {
            {
                __vtable: Box<#vtable>,
                __refs: std::sync::atomic::AtomicU32,
                #named
            }
        }
    }
}

use syn::MethodSig;

struct Implementation {
    // The identifiers of the interface methods.
    fns: Vec<Ident>,
}

impl Fold for Implementation {
    fn fold_method_sig(&mut self, mut f: MethodSig) -> MethodSig {
        // Ensure the functions have the right ABI.
        f.abi = Some(parse_quote!(extern "system"));

        // Store the identifier for later.
        self.fns.push(f.ident.clone());

        f
    }
}
