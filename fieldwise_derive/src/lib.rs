extern crate proc_macro;

use proc_macro2::TokenStream as TokenStream2;
use proc_macro::TokenStream;
use quote::quote;
use syn::spanned::Spanned;
use syn::{parse_macro_input, Data, DataStruct, DataEnum, DataUnion, DeriveInput, Ident, Fields};

fn derive_struct(root_type: Ident, data: DataStruct) -> TokenStream2 {
    match data.fields {
        Fields::Named(fields) => {
            fields.named.iter().enumerate().map(|(index, field)| {
                let field_name = field.ident.clone().unwrap_or_else(|| {
                    // for tuple structs, use index as field name
                    Ident::new(&format!("{}", index), field.span())
                });

                let field_lens_name = Ident::new(&format!("{}__{}", root_type, field_name), field_name.span());
                let field_type = field.ty.clone();

                quote! {
                    #[derive(Clone)]
                    #[allow(non_camel_case_types)]
                    pub struct #field_lens_name<B: ::fieldwise::Path>(B);

                    impl<B: ::fieldwise::Path<Item = #root_type>> ::fieldwise::Path for #field_lens_name<B> {
                        type Root = B::Root;
                        type Item = #field_type;

                        fn access<'a>(&self, root: &'a <Self::Root as Path>::Item) -> Option<&'a Self::Item> {
                            Some(&self.0.access(root)?.#field_name)
                        }

                        fn access_mut<'a>(&self, root: &'a mut <Self::Root as Path>::Item) -> Option<&'a mut Self::Item> {
                            Some(&mut self.0.access_mut(root)?.#field_name)
                        }
                    }
                }
            }).collect()
        }
        _ => panic!()
    }
}

#[proc_macro_derive(Fieldwise)]
pub fn derive_fieldwise(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    let root_type = input.ident.clone();
    let root_name = Ident::new(&format!("{}__", input.ident), input.ident.span());
    let root_vis = input.vis;

    let root_decl = quote! {
        #[derive(Clone)]
        #[allow(non_camel_case_types)]
        #root_vis struct #root_name;

        impl ::fieldwise::Path for #root_name {
            type Root = #root_name;
            type Item = #root_type;

            fn access<'a>(&self, root: &'a <Self::Root as ::fieldwise::Path>::Item) -> Option<&'a Self::Item> {
                Some(root)
            }

            fn access_mut<'a>(&self, root: &'a mut <Self::Root as ::fieldwise::Path>::Item) -> Option<&'a mut Self::Item> {
                Some(root)
            }
        }
    };

    let path_impl = match input.data {
        Data::Struct(data) => derive_struct(root_type, data),
        Data::Enum(_) => panic!("cannot derive Fieldwise for enums"),
        Data::Union(_) => panic!("cannot derive Fieldwise for unions"),
    };

    TokenStream::from(quote! {
        #root_decl
        #path_impl
    })
}
