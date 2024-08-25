use proc_macro::TokenStream;
use proc_macro_crate::{crate_name, FoundCrate};
use syn::{parse_macro_input, DeriveInput, Data, Meta, Token, DataStruct, Path};
use syn::punctuated::Punctuated;
use quote::quote;

// references: https://github.com/nazmulidris/rust-scratch/blob/main/macros/my_proc_macros_lib/src/builder.rs
// https://stackoverflow.com/a/76687540

#[proc_macro_derive(StoreContainer, attributes(quarve))]
pub fn store_container_derive(input: TokenStream) -> TokenStream {
    let input: DeriveInput = parse_macro_input!(input as DeriveInput);

    let ident = input.ident;
    let generics = input.generics;

    if let Data::Struct(strct) = input.data {
        let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

        let doc_str = format!(
            "Implements StoreContainer for [`{}`].\n ",
            &ident
        );

        let sub_stores: Vec<_> = filter_ignored_stores(&strct)
            .map(|field| field.ident.as_ref().unwrap())
            .collect();

        let sub_stores_clone = sub_stores.clone();

        let quarve_path: Path = match crate_name("quarve").expect("Error finding crate name") {
            FoundCrate::Itself => syn::parse_quote!(crate),
            FoundCrate::Name(_) => syn::parse_quote!(::quarve),
        };

        quote! {
            #[doc = #doc_str]
            impl #impl_generics #quarve_path::state::StoreContainer for #ident #ty_generics #where_clause {
                fn subtree_general_listener<__F_SC_FUNC: #quarve_path::state::GeneralListener + Clone>(&self, f: __F_SC_FUNC, s: #quarve_path::core::Slock<impl #quarve_path::util::marker::ThreadMarker>) {
                    #(self.#sub_stores.subtree_general_listener(f.clone(), s);)*
                }

                fn subtree_inverse_listener<__F_SC_FUNC: #quarve_path::state::InverseListener + Clone>(&self, f: __F_SC_FUNC, s: #quarve_path::core::Slock<impl #quarve_path::util::marker::ThreadMarker>) {
                    #(self.#sub_stores_clone.subtree_inverse_listener(f.clone(), s);)*
                }
            }
        }
    } else {
        panic!("Can only derive StoreContainer for a struct");
    }
        .into()
}

fn filter_ignored_stores(data: &DataStruct) -> impl Iterator<Item=&syn::Field> {
    data.fields.iter()
        .filter(
            |field| field.attrs.iter()
                .all(|attr| {
                    match &attr.meta {
                        Meta::List(lst) if lst.path.is_ident("quarve") => {
                            // panic!("{}", lst.tokens.to_string());
                            lst.parse_args_with(Punctuated::<syn::Ident, Token![,]>::parse_terminated)
                                .map(|val| val.iter().all(|lit| lit != "ignore"))
                                .unwrap_or_else(|err| {
                                    panic!("Invalid use of `quarve` attribute. Expected #[quarve(ignore)] {}", err.to_string())
                                })
                        },
                        _ => true
                    }
                })
        )
}