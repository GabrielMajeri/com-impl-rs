#![recursion_limit = "128"]

extern crate proc_macro;

#[macro_use]
extern crate quote;

#[macro_use]
extern crate syn;

use heck::CamelCase;

use self::proc_macro::TokenStream;

use syn::fold::Fold;
use syn::{Ident, ItemStruct, Fields, FieldsNamed};

#[proc_macro_attribute]
pub fn interface(attr: TokenStream, input: TokenStream) -> TokenStream {
    let iface: Ident = syn::parse(attr)
        .expect("You must specify the interface to implement");

    // Return the original input if it fails to parse.
    let input: syn::ItemStruct = match syn::parse(input.clone()) {
        Ok(input) => input,
        Err(_) => return input,
    };

    let vtable = make_vtable_ident(&iface);

    let mut iunknown = IUnknownImpl { vtable };
    let input = iunknown.fold_item_struct(input);

    let expanded = quote!(#input);

    expanded.into()
}

#[proc_macro_attribute]
pub fn implementation(attr: TokenStream, input: TokenStream) -> TokenStream {
    let iface: Ident = syn::parse(attr)
        .expect("Failed to parse implemented interface name");

    let vtable = make_vtable_ident(&iface);

    let input: syn::ItemImpl = syn::parse(input).expect("Could not parse interface impl block");

    let mut imp = Implementation { fns: Vec::new() };

    let input = imp.fold_item_impl(input);

    let self_ty = &input.self_ty;

    let fns = &imp.fns;
    let methods = fns.iter().map(|id| {
        let method_name = id.to_string().to_camel_case();
        syn::Ident::new(&method_name, id.span())
    });

    let parent: Option<syn::FieldValue> = if iface == "IUnknown" {
        None
    } else {
        Some(parse_quote!(parent: Self::create_vtable()))
    };

    let vtable_creator = quote! {
        impl com_impl::ComInterface<#vtable> for #self_ty {
            fn create_vtable() -> #vtable {
                use com_impl::ComInterface;
                unsafe {
                    #vtable {
                        #(#parent,)*
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

fn make_vtable_ident(iface: &Ident) -> Ident {
    let vtable_name = format!("{}Vtbl", iface);
    Ident::new(&vtable_name, iface.span())
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
