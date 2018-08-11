#![recursion_limit = "128"]

#[macro_use]
extern crate quote;

#[macro_use]
extern crate syn;

use heck::CamelCase;

use proc_macro::TokenStream;

use syn::punctuated::Punctuated;
use syn::synom::Synom;
use syn::Ident;

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

fn make_vtable_ident(ident: &syn::Ident) -> syn::Ident {
    let name = format!("{}Vtbl", ident);

    syn::Ident::new(&name, ident.span())
}

fn make_vtable_creator_ident(ident: &syn::Ident) -> syn::Ident {
    let name = format!("_create_{}", ident);

    syn::Ident::new(&name, ident.span())
}

#[proc_macro_attribute]
pub fn com_interface(attr: TokenStream, input: TokenStream) -> TokenStream {
    let Args { parents } =
        syn::parse(attr).expect("You must specify at least one interface to implement");

    assert_eq!(
        parents[0], "IUnknown",
        "First parent interface must always be IUnknown"
    );

    let last = parents.last().unwrap();
    let vtable = make_vtable_ident(&last);

    // Return the original input if it fails to parse.
    let mut input: syn::ItemStruct = match syn::parse(input.clone()) {
        Ok(input) => input,
        Err(_) => return input,
    };

    // Ensure the layout of the struct is fixed.
    let repr_c = parse_quote!(#[repr(C)]);
    input.attrs.push(repr_c);

    if let syn::Fields::Named(fnamed) = &mut input.fields {
        let fields = &fnamed.named;
        *fnamed = parse_quote! {
            {
                __vtable: Box<#vtable>,
                __refs: std::sync::atomic::AtomicU32,
                #fields
            }
        };
    } else {
        panic!("Only structs with named fields are supported");
    }

    let struct_name = input.ident.clone();

    let iunknown_vtable_creator = {
        quote! {
            impl #struct_name {
                fn _create_IUnknownVtbl() -> IUnknownVtbl {
                    unsafe {
                        IUnknownVtbl {
                            QueryInterface: std::mem::transmute(&Self::query_interface),
                            AddRef: std::mem::transmute(&Self::add_ref),
                            Release: std::mem::transmute(&Self::release),
                        }
                    }
                }
            }
        }
    };

    let refs_impl = quote! {
        impl #struct_name {
            fn create_refs() -> std::sync::atomic::AtomicU32 {
                std::sync::atomic::AtomicU32::new(1)
            }
        }
    };

    let iunknown_impl = quote! {
        impl #struct_name {
            fn query_interface(&mut self, riid: &winapi::shared::guiddef::GUID, obj: &mut usize) -> winapi::um::winnt::HRESULT {
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

            fn add_ref(&mut self) -> u32 {
                let prev = self.__refs.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                prev + 1
            }

            fn release(&mut self) -> u32 {
                let prev = self.__refs.fetch_sub(1, std::sync::atomic::Ordering::SeqCst);
                if prev == 1 {
                    let _box = unsafe { Box::from_raw(self as *mut _) };
                }
                prev - 1
            }
        }
    };

    let vtable_creator = {
        let last_vtable_creator = make_vtable_creator_ident(&vtable);
        quote! {
            impl #struct_name {
                fn create_vtable() -> Box<#vtable> {
                    Box::new(Self::#last_vtable_creator())
                }
            }
        }
    };

    let expanded = quote! {
        #input
        #refs_impl
        #iunknown_impl
        #iunknown_vtable_creator
        #vtable_creator
    };

    expanded.into()
}

#[proc_macro_attribute]
pub fn com_implementation(attr: TokenStream, input: TokenStream) -> TokenStream {
    let Args { parents } =
        syn::parse(attr).expect("Failed to parse parent interface and implemented interface");

    let parent = &parents[0];
    let iface = &parents[1];

    let mut input: syn::ItemImpl = syn::parse(input).expect("Could not parse interface impl block");

    let struct_name: syn::Ident = {
        let self_ty = input.self_ty.clone();
        syn::parse(quote!(#self_ty).into())
            .expect("The impl block should be for be the struct implementing the interface")
    };

    let vtable_creator = {
        let fns: Vec<_> = input
            .items
            .iter_mut()
            .filter_map(|it| match it {
                syn::ImplItem::Method(method) => {
                    method.sig.abi = Some(parse_quote!(extern "system"));

                    Some(&method.sig.ident)
                }
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
                            #(#method_names: std::mem::transmute(&Self::#fns),)*
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
