use std::borrow::BorrowMut;

use proc_macro::TokenStream;
use quote::quote;
use syn::{DataStruct, Field, Ident, Type};

#[proc_macro_derive(ToResponseSchema)]
pub fn to_response_schema(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).expect("Parse input to `to_response_schema`");
    impl_to_response_schema(&ast)
}

fn impl_to_response_schema(ast: &syn::DeriveInput) -> TokenStream {
    let struct_name = &ast.ident;
    let attrs = &ast.attrs;

    match &ast.data {
        syn::Data::Struct(named_data_structure) => {
            let gen = match &named_data_structure.fields {
                syn::Fields::Named(named_fields) => {
                    let types = named_fields.named.iter().map(|x| &(x.ty));
                    let names: Vec<Ident> = named_fields
                        .named
                        .iter()
                        .map(|x| (x.ident.clone().expect("Named field")))
                        .collect();
                    quote! {
                        impl ToResponseSchema<'_> for #struct_name {
                            fn to_response_schema() -> gemini::ResponseSchema {
                                let description = #struct_name::description();
                                ResponseSchema {
                                    schema_type: gemini::SchemaType::Object,
                                    format: None,
                                    description: if description.is_empty() {None} else {Some(description)},
                                    nullable: None,
                                    possibilities: None,
                                    max_items: None,
                                    properties: Some(std::collections::HashMap::from([
                                        #((stringify!(#names).to_string(), <#types>::to_response_schema())),*
                                    ])),
                                    required: Some(vec![#(stringify!(#names).to_string()),*]),
                                    items: None,
                                }
                            }
                        }
                    }
                }
                syn::Fields::Unnamed(unnamed_fields) => {
                    assert_eq!(unnamed_fields.unnamed.len(), 1);
                    let inner_type = &unnamed_fields.unnamed.iter().next().unwrap().ty;
                    quote! {
                        impl ToResponseSchema<'_> for #struct_name {
                            fn to_response_schema() -> gemini::ResponseSchema {
                                let mut response_schema = <#inner_type>::to_response_schema();
                                let description = <#struct_name>::description();
                                if !description.is_empty() {
                                    response_schema.set_description(description);
                                }
                                response_schema
                            }
                        }

                    }
                }
                syn::Fields::Unit => unimplemented!(),
            };
            gen.into()
        }
        syn::Data::Enum(_) => unimplemented!(),
        syn::Data::Union(_) => unimplemented!(),
    }
}
