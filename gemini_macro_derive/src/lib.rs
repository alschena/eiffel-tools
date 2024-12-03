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
    let data_struct = match &ast.data {
        syn::Data::Struct(a) => a,
        syn::Data::Enum(_) => unimplemented!(),
        syn::Data::Union(_) => unimplemented!(),
    };
    let gen = match &data_struct.fields {
        syn::Fields::Named(named_fields) => {
            let types = named_fields.named.iter().map(|x| x.ty.clone());
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
        syn::Fields::Unnamed(_) => unimplemented!(),
        syn::Fields::Unit => unimplemented!(),
    };
    gen.into()
}
