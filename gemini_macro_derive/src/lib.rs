use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_derive(ToResponseSchema)]
pub fn to_response_schema(input: TokenStream) -> TokenStream {
    let ast = syn::parse(input).expect("Parse input to `to_response_schema`");
    impl_to_response_schema(&ast)
}

fn impl_to_response_schema(ast: &syn::DeriveInput) -> TokenStream {
    todo!()
}
