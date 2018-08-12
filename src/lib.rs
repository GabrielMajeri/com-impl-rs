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
use syn::{Ident, ItemStruct, Fields, FieldsNamed, MethodSig};

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

struct IUnknownImpl {
    // The identifier of the last-level VTable.
    vtable: Ident,
}

impl Fold for IUnknownImpl {
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
        let vtable = &self.vtable;
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

fn make_vtable_ident(ident: &syn::Ident) -> syn::Ident {
    let name = format!("{}Vtbl", ident);

    syn::Ident::new(&name, ident.span())
}

fn make_vtable_creator_ident(ident: &syn::Ident) -> syn::Ident {
    let name = format!("__create_{}", ident);

    syn::Ident::new(&name, ident.span())
}

#[proc_macro_attribute]
pub fn interface(attr: TokenStream, input: TokenStream) -> TokenStream {
    let Args { parents } =
        syn::parse(attr).expect("You must specify at least one interface to implement");

    assert_eq!(
        parents[0], "IUnknown",
        "First parent interface must always be IUnknown"
    );

    let last = parents.last().unwrap();
    let vtable = make_vtable_ident(&last);

    // Return the original input if it fails to parse.
    let input: syn::ItemStruct = match syn::parse(input.clone()) {
        Ok(input) => input,
        Err(_) => return input,
    };

    let mut iunknown = IUnknownImpl { vtable: vtable.clone() };
    let input = iunknown.fold_item_struct(input);

    let struct_name = input.ident.clone();

    let vtable_creator: syn::ItemFn = {
        // Identifier of the function which generate the final vtable.
        let last_vtable_creator = make_vtable_creator_ident(&vtable);

        parse_quote! {
            fn create_vtable() -> Box<#vtable> {
                Box::new(Self::#last_vtable_creator())
            }
        }
    };


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

            fn __create_IUnknownVtbl() -> IUnknownVtbl {
                unsafe {
                    IUnknownVtbl {
                        QueryInterface: std::mem::transmute(Self::query_interface as usize),
                        AddRef: std::mem::transmute(Self::add_ref as usize),
                        Release: std::mem::transmute(Self::release as usize),
                    }
                }
            }

            #vtable_creator
        }
    };

    expanded.into()
}

struct SystemAbi;
impl Fold for SystemAbi {
    fn fold_method_sig(&mut self, mut f: MethodSig) -> MethodSig {
        f.abi = Some(parse_quote!(extern "system"));

        f
    }
}

#[proc_macro_attribute]
pub fn implementation(attr: TokenStream, input: TokenStream) -> TokenStream {
    let Args { parents } =
        syn::parse(attr).expect("Failed to parse parent interface and implemented interface");

    let parent = &parents[0];
    let iface = &parents[1];

    let input: syn::ItemImpl = syn::parse(input).expect("Could not parse interface impl block");

    let input = SystemAbi.fold_item_impl(input);

    let struct_name: syn::Ident = {
        let self_ty = input.self_ty.clone();
        syn::parse(quote!(#self_ty).into())
            .expect("The impl block should be for be the struct implementing the interface")
    };

    let vtable_creator = {
        let fns: Vec<_> = input.items.iter()
            .filter_map(|it| match it {
                syn::ImplItem::Method(method) => Some(&method.sig.ident),
                _ => None,
            }).collect();

        let method_names: Vec<_> = fns
            .iter()
            .map(|fn_ident| {
                let ptr_name = fn_ident.to_string().to_camel_case();

                syn::Ident::new(&ptr_name, fn_ident.span())
            }).collect();

        let vtable = make_vtable_ident(&iface);
        let creator_name = make_vtable_creator_ident(&vtable);
        let parent_vtable = make_vtable_ident(&parent);
        let parent_creator = make_vtable_creator_ident(&parent_vtable);

        quote! {
            impl #struct_name {
                fn #creator_name() -> #vtable {
                    unsafe {
                        #vtable {
                            parent: Self::#parent_creator(),
                            #(#method_names: std::mem::transmute((Self::#fns) as usize),)*
                        }
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
